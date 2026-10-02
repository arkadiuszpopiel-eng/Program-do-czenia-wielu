//! Wbudowany terminal w aplikacji (kategoria `app-*`; docs/modules/ui-terminal/SPEC.md):
//! komendy `terminal_open/input/resize/close/list` nad `ui-terminal-impl::TerminalManager`
//! (ConPTY: `platform-windows-pty-impl::WinPty`; w testach `platform-fake::FakePty`).
//!
//! - **Gest użytkownika** (`ui_only::user_gesture`) powstaje wyłącznie tutaj, w obsłudze komend UI
//!   wywoływanych przez powłokę Tauri z kliknięcia/klawisza w panelu terminala; żadna agentka nie
//!   ma narzędzia terminala, a wyzwalacze i harmonogram go nie otwierają.
//! - **Strumień** VT trafia wyłącznie do [`FrameSink`] przekazanego w `terminal_open` (powłoka:
//!   `tauri::ipc::Channel<TerminalFrame>`) — nigdy do zdarzeń `alfa://events`, magistrali ani logów
//!   (ten crate nie zależy od `tracing`); na magistralę idą tylko zdarzenia cyklu życia bez treści.
//! - **Programy profili**: PowerShell 7 (albo 5.1), `cmd.exe`, `claude`/`codex` z wykrycia CLI mostów
//!   (`CliProbe`) — sprawdzane przy każdym otwarciu (CLI zainstalowane później działa bez restartu).
//!   Polecenie logowania wpisuje użytkownik; Alfa nie czyta strumienia ani poświadczeń CLI.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use accounts_hub_contract::CliProbe;
use app_api::AppError;
use app_api::dto::{TerminalFrame, TerminalProfileId, TerminalSession};
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use core_bus_contract::EventBus;
use platform_contract::{PseudoConsolePort, PtySize};
use ui_terminal_contract::{
    MAX_INPUT_BYTES, OpenRequest, TerminalConfig, TerminalError, TerminalId, TerminalProfile,
    TerminalPrograms, TerminalService, TerminalSink, ui_only,
};
use ui_terminal_impl::TerminalManager;

/// Treść `module.toml` modułu `ui-terminal` (rejestr).
pub const MODULE_TOML: &str = ui_terminal_impl::MODULE_TOML;

/// Manifesty modułów składanych przez ten crate (identyfikator → `module.toml`).
pub const MODULES: &[(&str, &str)] = &[("ui-terminal", ui_terminal_impl::MODULE_TOML)];

/// Odbiorca ramek strumienia — kanał UI otwarcia (powłoka: `tauri::ipc::Channel`).
pub trait FrameSink: Send + Sync {
    /// Ramka (wyjście base64 albo koniec procesu).
    fn send(&self, frame: TerminalFrame);
}

impl<F: Fn(TerminalFrame) + Send + Sync> FrameSink for F {
    fn send(&self, frame: TerminalFrame) {
        self(frame);
    }
}

/// `TerminalSink` → ramki kanału UI (bez buforowania i kopii treści).
struct Frames(Arc<dyn FrameSink>);

impl TerminalSink for Frames {
    fn output(&self, _id: TerminalId, chunk: &[u8]) {
        self.0.send(TerminalFrame::Output {
            data_b64: STANDARD.encode(chunk),
        });
    }
    fn exited(&self, _id: TerminalId, code: Option<i32>) {
        self.0.send(TerminalFrame::Exit { code });
    }
}

/// Wykrywanie programów profili.
pub struct Programs {
    probe: Option<Arc<dyn CliProbe>>,
    fixed: Option<TerminalPrograms>,
}

impl Programs {
    /// Powłoki z systemu, CLI mostów z `probe`.
    pub fn detect(probe: Option<Arc<dyn CliProbe>>) -> Self {
        Self { probe, fixed: None }
    }

    /// Stałe ścieżki (testy).
    pub fn fixed(programs: TerminalPrograms) -> Self {
        Self {
            probe: None,
            fixed: Some(programs),
        }
    }

    fn resolve(&self) -> TerminalPrograms {
        if let Some(p) = &self.fixed {
            return p.clone();
        }
        let locate = |name: &str| self.probe.as_ref().and_then(|p| p.locate(name));
        let root = std::env::var_os("SystemRoot")
            .map_or_else(|| PathBuf::from(r"C:\Windows"), PathBuf::from);
        let powershell = root.join(r"System32\WindowsPowerShell\v1.0\powershell.exe");
        TerminalPrograms {
            shell: locate("pwsh").or_else(|| powershell.is_file().then_some(powershell)),
            cmd: std::env::var_os("ComSpec")
                .map(PathBuf::from)
                .or_else(|| Some(root.join(r"System32\cmd.exe")).filter(|p| p.is_file())),
            claude: locate("claude"),
            codex: locate("codex"),
        }
    }
}

fn profile(p: TerminalProfileId) -> TerminalProfile {
    match p {
        TerminalProfileId::Shell => TerminalProfile::Shell,
        TerminalProfileId::Cmd => TerminalProfile::Cmd,
        TerminalProfileId::ClaudeLogin => TerminalProfile::ClaudeLogin,
        TerminalProfileId::CodexLogin => TerminalProfile::CodexLogin,
    }
}

fn profile_id(p: TerminalProfile) -> TerminalProfileId {
    match p {
        TerminalProfile::Shell => TerminalProfileId::Shell,
        TerminalProfile::Cmd => TerminalProfileId::Cmd,
        TerminalProfile::ClaudeLogin => TerminalProfileId::ClaudeLogin,
        TerminalProfile::CodexLogin => TerminalProfileId::CodexLogin,
    }
}

fn error(e: TerminalError) -> AppError {
    match e {
        TerminalError::NotFound(id) => AppError::not_found(format!("Terminal {id} nie istnieje.")),
        TerminalError::Limit(n) => AppError::invalid(format!(
            "Otwarto już {n} terminale — zamknij któryś, zanim otworzysz kolejny."
        )),
        TerminalError::ProgramMissing(p) => AppError::unavailable(
            &format!("Terminal ({p}) — program nie jest zainstalowany"),
            "ui-terminal",
        ),
        TerminalError::InputTooLarge => {
            AppError::invalid(format!("Paczka wejścia większa niż {MAX_INPUT_BYTES} B."))
        }
        TerminalError::Platform(m) => AppError::internal(format!("terminal: {m}")),
    }
}

/// Terminale aplikacji.
pub struct TerminalApp {
    pty: Arc<dyn PseudoConsolePort>,
    programs: Programs,
    default_cwd: PathBuf,
    bus: Option<(Arc<dyn EventBus>, tokio::runtime::Handle)>,
    manager: Mutex<Option<(TerminalPrograms, Arc<TerminalManager>)>>,
}

impl std::fmt::Debug for TerminalApp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TerminalApp")
            .field("sessions", &self.list().len())
            .finish_non_exhaustive()
    }
}

impl TerminalApp {
    /// Terminale nad pseudokonsolą (`None` = ConPTY systemu).
    pub fn new(
        pty: Option<Arc<dyn PseudoConsolePort>>,
        programs: Programs,
        default_cwd: PathBuf,
    ) -> Self {
        Self {
            pty: pty.unwrap_or_else(|| Arc::new(platform_windows_pty_impl::WinPty)),
            programs,
            default_cwd,
            bus: None,
            manager: Mutex::new(None),
        }
    }

    /// Zdarzenia cyklu życia (bez treści) na magistrali (builder).
    #[must_use]
    pub fn with_bus(mut self, bus: Arc<dyn EventBus>, handle: tokio::runtime::Handle) -> Self {
        self.bus = Some((bus, handle));
        self
    }

    fn lock(&self) -> MutexGuard<'_, Option<(TerminalPrograms, Arc<TerminalManager>)>> {
        self.manager.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Menedżer z aktualnymi programami (nowy, gdy programy się zmieniły i nic nie jest otwarte).
    fn manager(&self) -> Arc<TerminalManager> {
        let programs = self.programs.resolve();
        let mut slot = self.lock();
        if let Some((current, m)) = slot.as_ref()
            && (*current == programs || !m.list().is_empty())
        {
            return m.clone();
        }
        let config = TerminalConfig {
            programs: programs.clone(),
            default_cwd: self.default_cwd.clone(),
            ..TerminalConfig::default()
        };
        let mut manager = TerminalManager::new(self.pty.clone(), config);
        if let Some((bus, handle)) = &self.bus {
            manager = manager.with_bus(bus.clone(), handle.clone());
        }
        let manager = Arc::new(manager);
        *slot = Some((programs, manager.clone()));
        manager
    }

    fn current(&self) -> Option<Arc<TerminalManager>> {
        self.lock().as_ref().map(|(_, m)| m.clone())
    }

    /// `terminal_open` — wyłącznie z gestu użytkownika w UI (komenda powłoki).
    pub fn open(
        &self,
        profile_id: TerminalProfileId,
        cols: u16,
        rows: u16,
        cwd: Option<String>,
        sink: Arc<dyn FrameSink>,
    ) -> Result<TerminalSession, AppError> {
        let size = PtySize { cols, rows };
        size.validate()
            .map_err(|e| AppError::invalid(format!("Rozmiar terminala: {e}")))?;
        let cwd = cwd.map(PathBuf::from).filter(|p| p.is_dir());
        let manager = self.manager();
        let request = OpenRequest {
            profile: profile(profile_id),
            size,
            cwd,
        };
        let gesture = ui_only::user_gesture();
        let id = manager
            .open(request, &gesture, Arc::new(Frames(sink)))
            .map_err(error)?;
        let info = manager
            .list()
            .into_iter()
            .find(|i| i.id == id)
            .ok_or_else(|| AppError::internal("terminal zamknął się przy starcie"))?;
        Ok(session(&info))
    }

    /// `terminal_input` — klawiatura panelu terminala (base64, ≤ 64 KiB po zdekodowaniu).
    pub fn input(&self, terminal: u64, data_b64: &str) -> Result<(), AppError> {
        let data = STANDARD
            .decode(data_b64.as_bytes())
            .map_err(|_| AppError::invalid("Wejście terminala nie jest poprawnym base64."))?;
        let manager = self
            .current()
            .ok_or_else(|| AppError::not_found(format!("Terminal {terminal} nie istnieje.")))?;
        let gesture = ui_only::user_gesture();
        manager
            .input(TerminalId(terminal), &data, &gesture)
            .map_err(error)
    }

    /// `terminal_resize`.
    pub fn resize(&self, terminal: u64, cols: u16, rows: u16) -> Result<(), AppError> {
        let manager = self
            .current()
            .ok_or_else(|| AppError::not_found(format!("Terminal {terminal} nie istnieje.")))?;
        manager
            .resize(TerminalId(terminal), PtySize { cols, rows })
            .map_err(error)
    }

    /// `terminal_close` — zabija drzewo procesów.
    pub fn close(&self, terminal: u64) -> Result<(), AppError> {
        let manager = self
            .current()
            .ok_or_else(|| AppError::not_found(format!("Terminal {terminal} nie istnieje.")))?;
        manager.close(TerminalId(terminal)).map_err(error)
    }

    /// `terminal_list` — bez treści.
    pub fn list(&self) -> Vec<TerminalSession> {
        self.current()
            .map(|m| m.list().iter().map(session).collect())
            .unwrap_or_default()
    }

    /// Zamyka wszystkie terminale (kill-switch nie dotyczy terminala użytkownika; zamknięcie
    /// aplikacji — tak).
    pub fn close_all(&self) {
        if let Some(m) = self.current() {
            for s in m.list() {
                let _ = m.close(s.id);
            }
        }
    }
}

fn session(info: &ui_terminal_contract::TerminalInfo) -> TerminalSession {
    TerminalSession {
        id: info.id.0,
        profile: profile_id(info.profile),
        pid: info.pid,
        alive: info.alive,
    }
}
