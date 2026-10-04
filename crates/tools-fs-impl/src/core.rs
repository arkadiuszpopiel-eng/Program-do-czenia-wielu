//! Rdzeń wspólny narzędzi plikowych: ścieżki z deny-listą przed Brokerem, zgody Brokera,
//! kroki dziennika cofania, mapowanie błędów platformy na wyniki dla modelu, zdarzenia.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use compliance_contract::{DenyChecker, DenyLists, PathEnv};
use core_bus_contract::{EventBus, Level};
use platform_contract::{FsPort, PlatformError};
use safety_broker_contract::{Broker, Capability, DeclaredFacts, PathScope};
use tools_common_contract::{
    Authorization, BrokerGate, DenialReason, ToolCtx, ToolErrorKind, ToolManifest, ToolOutcome,
    UndoRef, UndoService, action_request, base_facts, paths, text, tool_event,
};
use tools_fs_contract::{EVENT_DENIED, EVENT_DENYLIST_HIT, EVENT_RESULT, FsToolsConfig};
use undo_journal_contract::{StepCtx, StepId, UndoError, UndoJournal};

/// Zależności narzędzi plikowych.
#[derive(Clone)]
pub struct FsToolsDeps {
    /// Port systemu plików.
    pub fs: Arc<dyn FsPort>,
    /// Dziennik cofania (każda mutacja).
    pub journal: Arc<dyn UndoJournal>,
    /// Broker (tokeny, zatwierdzenia, taint).
    pub broker: Arc<dyn Broker>,
    /// Środowisko ścieżek (profil właściciela).
    pub env: PathEnv,
    /// Deny-listy Jądra (współdzielone z `[platform.denylist_paths]`).
    pub deny: DenyLists,
    /// Limity.
    pub config: FsToolsConfig,
    /// Magistrala (zdarzenia `tool.fs.*`, best effort).
    pub bus: Option<Arc<dyn EventBus>>,
}

/// Rdzeń: zależności skompilowane do szybkich sprawdzeń.
pub(crate) struct Core {
    pub(crate) fs: Arc<dyn FsPort>,
    pub(crate) journal: Arc<dyn UndoJournal>,
    pub(crate) gate: BrokerGate,
    pub(crate) env: PathEnv,
    pub(crate) deny: DenyChecker,
    pub(crate) config: FsToolsConfig,
    pub(crate) bus: Option<Arc<dyn EventBus>>,
}

/// Wynik pośredni: `Err` to gotowy wynik dla modelu (odmowa, błąd).
pub(crate) type Step<T> = Result<T, Box<ToolOutcome>>;

impl Core {
    pub(crate) fn new(deps: FsToolsDeps) -> Self {
        Self {
            deny: DenyChecker::new(deps.deny, &deps.env),
            fs: deps.fs,
            journal: deps.journal,
            gate: BrokerGate::new(deps.broker),
            env: deps.env,
            config: deps.config,
            bus: deps.bus,
        }
    }

    pub(crate) async fn emit(
        &self,
        name: &str,
        level: Level,
        payload: serde_json::Value,
        ctx: &ToolCtx,
    ) {
        if let Some(bus) = &self.bus {
            let _ = bus.publish(tool_event(name, level, payload, ctx)).await;
        }
    }

    /// Czy ścieżka jest na deny-liście (Jądra albo poświadczeń platformy) — także po rozwiązaniu
    /// dowiązań, przy każdym wywołaniu (symlink/junction w katalogu roboczym; Q-1).
    pub(crate) fn denied(&self, path: &str) -> bool {
        paths::protected_with_links(path, |p| {
            self.deny.is_denied_path(p, &self.env) || paths::has_credential_segment(p)
        })
    }

    /// Ścieżka od modelu → ścieżka sprawdzona (postać, deny-lista — bez pytania Brokera).
    pub(crate) async fn resolve(&self, raw: &str, ctx: &ToolCtx, m: &ToolManifest) -> Step<String> {
        let path = paths::resolve_path(raw, ctx.workdir.as_deref(), &self.env).map_err(|e| {
            Box::new(ToolOutcome::failed(
                ToolErrorKind::InvalidArgs,
                format!("Niepoprawna ścieżka: {e}."),
            ))
        })?;
        if self.denied(&path) {
            let payload = serde_json::json!({ "tool": m.id, "path": path });
            self.emit(EVENT_DENYLIST_HIT, Level::Warn, payload, ctx)
                .await;
            return Err(Box::new(ToolOutcome::denied(
                DenialReason::DenyList,
                &format!("{} „{path}”", m.title.to_lowercase()),
            )));
        }
        Ok(path)
    }

    pub(crate) fn scope(&self, path: &str, tree: bool) -> Step<PathScope> {
        let scope = if tree {
            paths::tree_scope(path, &self.env)
        } else {
            paths::exact_scope(path, &self.env)
        };
        scope.map_err(|e| {
            Box::new(ToolOutcome::failed(
                ToolErrorKind::InvalidArgs,
                format!("Niepoprawna ścieżka: {e}."),
            ))
        })
    }

    /// Zgody Brokera na zdolności (po kolei) i weryfikacja każdego użycia.
    pub(crate) async fn authorize(
        &self,
        ctx: &ToolCtx,
        m: &ToolManifest,
        caps: Vec<(Capability, DeclaredFacts)>,
        action: &str,
    ) -> Step<Vec<Authorization>> {
        let needed: Vec<Capability> = caps.iter().map(|(c, _)| c.clone()).collect();
        let requests = caps
            .into_iter()
            .map(|(c, f)| action_request(ctx, c, f))
            .collect();
        let auths = match self.gate.authorize_all(requests, ctx).await {
            Ok(a) => a,
            Err(e) => {
                let payload = serde_json::json!({ "tool": m.id, "error": e.to_string() });
                self.emit(EVENT_DENIED, Level::Warn, payload, ctx).await;
                return Err(Box::new(e.into_outcome(action)));
            }
        };
        for (auth, cap) in auths.iter().zip(&needed) {
            if let Err(e) = self.gate.verify(auth, cap, &ctx.holder) {
                self.gate.release(&auths).await;
                return Err(Box::new(e.into_outcome(action)));
            }
        }
        Ok(auths)
    }

    /// Fakty bazowe narzędzia.
    pub(crate) fn facts(&self, m: &ToolManifest, ctx: &ToolCtx) -> DeclaredFacts {
        base_facts(m, ctx)
    }

    /// Krok dziennika cofania dla mutacji.
    pub(crate) fn begin(
        &self,
        ctx: &ToolCtx,
        m: &ToolManifest,
        irreversible: bool,
    ) -> Step<StepId> {
        let step_ctx = StepCtx {
            session: ctx.holder.session.clone(),
            agent: ctx.holder.agent.clone(),
            run: ctx.run.clone(),
            turn: None,
            label: ctx.undo_label(m),
            allow_irreversible: irreversible,
        };
        self.journal
            .begin_step(step_ctx)
            .map_err(|e| Box::new(undo_failure(&e, &m.title)))
    }

    /// Zatwierdza krok (sukces) albo przerywa go (błąd operacji — nic nie zostaje).
    pub(crate) fn finish(
        &self,
        step: StepId,
        op: Result<(), UndoError>,
        m: &ToolManifest,
    ) -> Step<UndoRef> {
        if let Err(e) = op {
            let _ = self.journal.abort_step(step);
            return Err(Box::new(undo_failure(&e, &m.title)));
        }
        match self.journal.commit_step(step) {
            Ok(summary) => Ok(UndoRef {
                service: UndoService::Journal,
                id: step.0,
                text: summary.text,
            }),
            Err(e) => {
                let _ = self.journal.abort_step(step);
                Err(Box::new(undo_failure(&e, &m.title)))
            }
        }
    }

    /// Zakończenie wywołania: zdarzenie wyniku, unieważnienie tokenów.
    pub(crate) async fn done(
        &self,
        ctx: &ToolCtx,
        m: &ToolManifest,
        auths: &[Authorization],
        mut out: ToolOutcome,
    ) -> ToolOutcome {
        self.gate.release(auths).await;
        if out.approval.is_none() {
            out.approval = auths.iter().find_map(|a| a.approval);
        }
        let (body, cut) = text::truncate_chars(&out.text, self.config.output_max_chars);
        out.text = body;
        out.truncated |= cut;
        let payload = serde_json::json!({
            "tool": m.id, "status": out.status, "undo": out.undo, "approval": out.approval,
        });
        self.emit(EVENT_RESULT, Level::Info, payload, ctx).await;
        out
    }

    pub(crate) fn path_buf(path: &str) -> PathBuf {
        PathBuf::from(path)
    }

    /// Rodzaj istniejącej ścieżki: `Some((is_dir, size))` z listy katalogu nadrzędnego
    /// (zapas: odczyt pliku), `None` gdy nie istnieje.
    pub(crate) fn kind_of(&self, path: &Path) -> Option<(bool, u64)> {
        if !self.fs.exists(path) {
            return None;
        }
        let listed = path
            .parent()
            .and_then(|parent| self.fs.list_dir(parent).ok())
            .and_then(|entries| entries.into_iter().find(|e| e.path == path));
        Some(match listed {
            Some(e) => (e.is_dir, e.size),
            None => match self.fs.read(path) {
                Ok(d) => (false, d.len() as u64),
                Err(_) => (true, 0),
            },
        })
    }
}

/// Błąd platformy → wynik dla modelu.
pub(crate) fn platform_failure(e: &PlatformError, action: &str) -> ToolOutcome {
    let kind = match e {
        PlatformError::Denylisted(_) => {
            return ToolOutcome::denied(DenialReason::DenyList, action);
        }
        PlatformError::NotFound(_) => ToolErrorKind::NotFound,
        PlatformError::AlreadyExists(_) => ToolErrorKind::AlreadyExists,
        PlatformError::InvalidPath(_) => ToolErrorKind::InvalidArgs,
        PlatformError::Unsupported(_) => ToolErrorKind::Unsupported,
        _ => ToolErrorKind::Io,
    };
    ToolOutcome::failed(kind, format!("Nie wykonano: {action} — {e}."))
}

/// Błąd dziennika → wynik dla modelu.
pub(crate) fn undo_failure(e: &UndoError, action: &str) -> ToolOutcome {
    match e {
        UndoError::Platform(p) => platform_failure(p, action),
        UndoError::PreImageTooLarge { .. } => ToolOutcome::failed(
            ToolErrorKind::Io,
            format!("Nie wykonano: {action} — {e}. Poproś właściciela o wykonanie ręcznie."),
        ),
        other => ToolOutcome::failed(
            ToolErrorKind::Internal,
            format!("Nie wykonano: {action} — dziennik cofania: {other}."),
        ),
    }
}
