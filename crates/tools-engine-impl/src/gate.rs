//! Zgody Brokera dla narzędzi silnika, ścieżki plików zapisu świata i błędy dla modelu.

use std::path::PathBuf;

use safety_broker_contract::{AppSelector, Capability, PathScope};
use tools_common_contract::{
    Authorization, DenialReason, ToolCtx, ToolErrorKind, ToolManifest, ToolOutcome, action_request,
    base_facts,
};
use tools_engine_contract::{Access, ENGINE_APP, EngineError, access, hint};

use crate::Core;

pub(crate) type Step<T> = Result<T, Box<ToolOutcome>>;

pub(crate) fn fail(kind: ToolErrorKind, text: impl Into<String>) -> Box<ToolOutcome> {
    Box::new(ToolOutcome::failed(kind, text))
}

/// Błąd silnika jako wynik dla modelu (z podpowiedzią, co zrobić dalej).
pub(crate) fn engine_fail(e: &EngineError) -> Box<ToolOutcome> {
    let kind = match e {
        EngineError::NotConfigured | EngineError::NotInstalled(_) => ToolErrorKind::Unsupported,
        EngineError::NotRunning => ToolErrorKind::NotFound,
        EngineError::AlreadyRunning(_) => ToolErrorKind::AlreadyExists,
        EngineError::Timeout(_) => ToolErrorKind::Timeout,
        EngineError::TooLarge { .. } => ToolErrorKind::InvalidArgs,
        EngineError::Rpc { code, kind, .. }
            if *code == -32602 || kind == "invalid_input" || kind == "invalid_params" =>
        {
            ToolErrorKind::InvalidArgs
        }
        EngineError::Rpc { kind, .. } if kind == "entity_not_found" => ToolErrorKind::NotFound,
        _ => ToolErrorKind::Io,
    };
    let text = match e {
        EngineError::Rpc { kind, .. } => format!("Silnik: {e}. {}", hint(kind)),
        _ => format!("Silnik: {e}."),
    };
    fail(kind, text)
}

impl Core {
    fn root_scope(&self) -> Step<PathScope> {
        let root = self
            .port
            .installation()
            .root
            .ok_or_else(|| engine_fail(&EngineError::NotConfigured))?;
        PathScope::tree(&root.display().to_string(), &self.env)
            .map_err(|e| fail(ToolErrorKind::InvalidArgs, format!("Katalog silnika: {e}.")))
    }

    async fn ask(
        &self,
        ctx: &ToolCtx,
        m: &ToolManifest,
        cap: Capability,
        command: Option<String>,
        action: &str,
    ) -> Step<Authorization> {
        let mut facts = base_facts(m, ctx);
        facts.command = command;
        let auth = self
            .gate
            .authorize(action_request(ctx, cap.clone(), facts), ctx)
            .await
            .map_err(|e| Box::new(e.into_outcome(action)))?;
        if let Err(e) = self.gate.verify(&auth, &cap, &ctx.holder) {
            self.gate.release(std::slice::from_ref(&auth)).await;
            return Err(Box::new(e.into_outcome(action)));
        }
        Ok(auth)
    }

    /// Zgoda na narzędzie według jego rodzaju (`command` — opis na karcie zatwierdzenia).
    pub(crate) async fn authorize(
        &self,
        ctx: &ToolCtx,
        m: &ToolManifest,
        command: Option<String>,
    ) -> Step<Authorization> {
        let action = m.title.to_lowercase();
        let cap = match access(&m.name) {
            Access::Read => Capability::FsRead(self.root_scope()?),
            Access::Process => Capability::ShellExec(self.root_scope()?),
            Access::Query | Access::Change => AppSelector::parse(ENGINE_APP)
                .map(Capability::GuiControl)
                .map_err(|e| fail(ToolErrorKind::Internal, format!("Aplikacja silnika: {e}.")))?,
        };
        self.ask(ctx, m, cap, command, &action).await
    }

    /// Plik zapisu świata (`load`): względny — wobec katalogu roboczego sesji; deny-lista;
    /// osobna zgoda na odczyt pliku.
    pub(crate) async fn load_path(
        &self,
        ctx: &ToolCtx,
        m: &ToolManifest,
        raw: Option<&str>,
    ) -> Step<(Option<PathBuf>, Option<Authorization>)> {
        let Some(raw) = raw else {
            return Ok((None, None));
        };
        let path = PathBuf::from(raw);
        let path = if path.is_absolute() {
            path
        } else {
            let dir = ctx.workdir.clone().ok_or_else(|| {
                fail(
                    ToolErrorKind::InvalidArgs,
                    "Ścieżka `load` jest względna, a sesja nie ma katalogu roboczego — podaj pełną ścieżkę.",
                )
            })?;
            PathBuf::from(dir).join(path)
        };
        let text = path.display().to_string();
        if self.deny.is_denied_path(&text, &self.env) {
            return Err(Box::new(ToolOutcome::denied(
                DenialReason::DenyList,
                "wczytanie zapisu świata z tego miejsca",
            )));
        }
        let scope = PathScope::exact(&text, &self.env)
            .map_err(|e| fail(ToolErrorKind::InvalidArgs, format!("Plik `load`: {e}.")))?;
        let auth = self
            .ask(
                ctx,
                m,
                Capability::FsRead(scope),
                Some(format!("wczytanie świata z {text}")),
                "odczyt zapisu świata",
            )
            .await?;
        Ok((Some(path), Some(auth)))
    }

    /// Zwalnia tokeny po akcji.
    pub(crate) async fn release(&self, auths: Vec<Authorization>) {
        self.gate.release(&auths).await;
    }
}
