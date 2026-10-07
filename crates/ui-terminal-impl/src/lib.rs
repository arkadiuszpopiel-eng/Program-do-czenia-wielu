//! `ui-terminal` — implementacja (docs/modules/ui-terminal/SPEC.md, PLAN §5.5–5.6, F4).
//!
//! `TerminalManager` nad `PseudoConsolePort`: profil → program (CLI z wykrycia `accounts-hub`),
//! jawne środowisko (`filter_env` — bez kluczy API i tokenów Alfy) + `TERM`, proces w Job Object.
//! Wyjście czyta osobny wątek i oddaje **wyłącznie** do `TerminalSink` (kanał UI) — bez buforów,
//! logów, magistrali i pamięci; na magistralę idą tylko zdarzenia cyklu życia bez treści. Wątek
//! nadzoru wykrywa koniec procesu (ConPTY nie kończy strumienia sam) i zamyka pseudokonsolę.
//! Zamknięcie i porzucenie menedżera zabijają drzewo procesów.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

mod session;

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

use core_bus_contract::{Event, EventBus, EventKind, Level};
use core_registry_contract::{ManifestError, ModuleManifest};
use platform_contract::{DEFAULT_ENV_ALLOWLIST, PseudoConsolePort, PtySize, PtySpec, filter_env};
use ui_terminal_contract::{
    EVENT_CLOSED, EVENT_OPENED, MAX_INPUT_BYTES, OpenRequest, TerminalConfig, TerminalError,
    TerminalId, TerminalInfo, TerminalProfile, TerminalService, TerminalSink, UserGesture,
};

use session::Session;

/// Treść `module.toml` tego modułu.
pub const MODULE_TOML: &str = include_str!("../module.toml");

/// Manifest modułu (rejestr).
pub fn module_manifest() -> Result<ModuleManifest, ManifestError> {
    ModuleManifest::parse_toml(MODULE_TOML)
}

/// Publikacja zdarzeń cyklu życia z dowolnego wątku (bez treści strumienia).
#[derive(Clone, Default)]
pub(crate) struct Publisher {
    bus: Option<(Arc<dyn EventBus>, tokio::runtime::Handle)>,
}

impl Publisher {
    pub(crate) fn publish(&self, name: &str, payload: serde_json::Value) {
        if let Some((bus, handle)) = &self.bus {
            let (bus, event) = (
                bus.clone(),
                Event::new(EventKind::Custom(name.to_owned()), Level::Info, payload),
            );
            handle.spawn(async move {
                let _ = bus.publish(event).await;
            });
        }
    }
}

/// Menedżer terminali (implementacja `TerminalService`).
pub struct TerminalManager {
    pty: Arc<dyn PseudoConsolePort>,
    config: TerminalConfig,
    env: Vec<(String, String)>,
    publisher: Publisher,
    sessions: Mutex<BTreeMap<TerminalId, Session>>,
    next: AtomicU64,
}

impl std::fmt::Debug for TerminalManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TerminalManager")
            .field("sessions", &self.lock().len())
            .finish_non_exhaustive()
    }
}

fn platform(e: &platform_contract::PlatformError) -> TerminalError {
    TerminalError::Platform(e.to_string())
}

impl TerminalManager {
    /// Menedżer nad portem pseudokonsoli; środowisko bazowe — środowisko procesu (filtrowane).
    pub fn new(pty: Arc<dyn PseudoConsolePort>, config: TerminalConfig) -> Self {
        Self {
            pty,
            config,
            env: std::env::vars().collect(),
            publisher: Publisher::default(),
            sessions: Mutex::new(BTreeMap::new()),
            next: AtomicU64::new(1),
        }
    }

    /// Własne środowisko bazowe (i tak filtrowane allowlistą bez sekretów).
    #[must_use]
    pub fn with_env(mut self, env: Vec<(String, String)>) -> Self {
        self.env = env;
        self
    }

    /// Zdarzenia cyklu życia na magistrali (publikowane w podanym środowisku tokio).
    #[must_use]
    pub fn with_bus(mut self, bus: Arc<dyn EventBus>, handle: tokio::runtime::Handle) -> Self {
        self.publisher = Publisher {
            bus: Some((bus, handle)),
        };
        self
    }

    fn lock(&self) -> MutexGuard<'_, BTreeMap<TerminalId, Session>> {
        self.sessions.lock().unwrap_or_else(|p| p.into_inner())
    }

    fn program(&self, profile: TerminalProfile) -> Result<(PathBuf, Vec<String>), TerminalError> {
        let p = &self.config.programs;
        let (path, args): (Option<&PathBuf>, &[&str]) = match profile {
            TerminalProfile::Shell => (p.shell.as_ref(), &["-NoLogo"]),
            TerminalProfile::Cmd => (p.cmd.as_ref(), &[]),
            TerminalProfile::ClaudeLogin => (p.claude.as_ref(), &[]),
            TerminalProfile::CodexLogin => (p.codex.as_ref(), &["login"]),
        };
        let path = path.ok_or_else(|| TerminalError::ProgramMissing(format!("{profile:?}")))?;
        Ok((path.clone(), args.iter().map(|a| (*a).to_owned()).collect()))
    }

    fn spec(&self, request: &OpenRequest) -> Result<PtySpec, TerminalError> {
        let (program, args) = self.program(request.profile)?;
        let mut env = filter_env(self.env.iter().cloned(), &DEFAULT_ENV_ALLOWLIST);
        env.retain(|(k, _)| {
            !k.eq_ignore_ascii_case("TERM") && !k.eq_ignore_ascii_case("COLORTERM")
        });
        env.push(("TERM".into(), "xterm-256color".into()));
        env.push(("COLORTERM".into(), "truecolor".into()));
        Ok(PtySpec {
            program,
            args,
            cwd: request
                .cwd
                .clone()
                .unwrap_or_else(|| self.config.default_cwd.clone()),
            env,
            size: request.size,
        })
    }
}

impl TerminalService for TerminalManager {
    fn open(
        &self,
        request: OpenRequest,
        _gesture: &UserGesture,
        sink: Arc<dyn TerminalSink>,
    ) -> Result<TerminalId, TerminalError> {
        let spec = self.spec(&request)?;
        let mut sessions = self.lock();
        if sessions.len() >= self.config.max_sessions.max(1) {
            return Err(TerminalError::Limit(self.config.max_sessions.max(1)));
        }
        let id = TerminalId(self.next.fetch_add(1, Ordering::SeqCst));
        let pty = self.pty.spawn(&spec).map_err(|e| platform(&e))?;
        let session = Session::start(
            id,
            request.profile,
            Arc::from(pty),
            sink,
            self.publisher.clone(),
        )
        .map_err(|e| platform(&e))?;
        let pid = session.pid();
        sessions.insert(id, session);
        drop(sessions);
        self.publisher.publish(
            EVENT_OPENED,
            serde_json::json!({ "terminal": id.0, "profile": request.profile, "pid": pid }),
        );
        Ok(id)
    }

    fn input(
        &self,
        id: TerminalId,
        data: &[u8],
        _gesture: &UserGesture,
    ) -> Result<(), TerminalError> {
        if data.len() > MAX_INPUT_BYTES {
            return Err(TerminalError::InputTooLarge);
        }
        let pty = self
            .lock()
            .get(&id)
            .map(Session::pty)
            .ok_or(TerminalError::NotFound(id.0))?;
        pty.write_input(data).map_err(|e| platform(&e))
    }

    fn resize(&self, id: TerminalId, size: PtySize) -> Result<(), TerminalError> {
        size.validate().map_err(|e| platform(&e))?;
        let pty = self
            .lock()
            .get(&id)
            .map(Session::pty)
            .ok_or(TerminalError::NotFound(id.0))?;
        pty.resize(size).map_err(|e| platform(&e))
    }

    fn close(&self, id: TerminalId) -> Result<(), TerminalError> {
        let session = self
            .lock()
            .remove(&id)
            .ok_or(TerminalError::NotFound(id.0))?;
        let result = session.close();
        self.publisher
            .publish(EVENT_CLOSED, serde_json::json!({ "terminal": id.0 }));
        result.map_err(|e| platform(&e))
    }

    fn list(&self) -> Vec<TerminalInfo> {
        self.lock().values().map(Session::info).collect()
    }
}

impl Drop for TerminalManager {
    fn drop(&mut self) {
        for (_, s) in std::mem::take(&mut *self.lock()) {
            let _ = s.close();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn module_manifest_parses() {
        assert_eq!(module_manifest().unwrap().id.as_str(), "ui-terminal");
    }
}
