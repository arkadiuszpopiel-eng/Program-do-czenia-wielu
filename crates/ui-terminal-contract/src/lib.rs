//! Kontrakt `ui-terminal` (docs/modules/ui-terminal/SPEC.md, PLAN §5.5–5.6, §16.2 F4; bramka
//! ludzka #1 „logowanie do CLI w wbudowanym terminalu”).
//!
//! Wbudowany terminal ConPTY, w którym **użytkownik sam** loguje się do CLI mostów (`claude`,
//! `codex login`). Zasady (testowane):
//! - sterowany wyłącznie przez użytkownika: otwarcie i wejście wymagają [`UserGesture`]
//!   (tworzonego tylko przez komendę UI w korzeniu kompozycji), moduł nie wystawia narzędzia
//!   agentek, nie zależy od `tools-*`/`agent-*`/`mcp-*`, a zależeć od niego może tylko `app-*`;
//! - strumień nie jest logowany ani zapisywany: wyjście VT idzie prosto do [`TerminalSink`] (kanał
//!   UI), na magistralę trafiają wyłącznie zdarzenia cyklu życia bez treści; `Debug` bez treści;
//! - środowisko procesu jawne i bez sekretów Alfy ([`platform_contract::filter_env`]);
//! - zamknięcie (i porzucenie usługi) zabija całe drzewo procesów (Job Object).

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

#[cfg(feature = "contract-tests")]
pub mod contract_tests;

use std::path::PathBuf;
use std::sync::Arc;

pub use platform_contract::PtySize;
use serde::{Deserialize, Serialize};

/// Zdarzenie: otwarto terminal (`terminal`, `profile`, `pid`) — bez treści.
pub const EVENT_OPENED: &str = "terminal.opened";
/// Zdarzenie: proces terminala zakończył się (`terminal`, `code`).
pub const EVENT_EXITED: &str = "terminal.exited";
/// Zdarzenie: terminal zamknięty przez użytkownika (`terminal`) — drzewo procesów zabite.
pub const EVENT_CLOSED: &str = "terminal.closed";
/// Maksymalna paczka wejścia z UI (B).
pub const MAX_INPUT_BYTES: usize = 64 * 1024;

/// Identyfikator sesji terminala.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct TerminalId(pub u64);

/// Profil: co uruchomić w terminalu.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TerminalProfile {
    /// PowerShell (7, a gdy brak — 5.1).
    Shell,
    /// `cmd.exe`.
    Cmd,
    /// Claude Code (`claude`; logowanie poleceniem `/login` w CLI).
    ClaudeLogin,
    /// Codex (`codex login`).
    CodexLogin,
}

/// Programy profili (ścieżki bezwzględne; CLI mostów z wykrycia w `accounts-hub`).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TerminalPrograms {
    /// PowerShell.
    pub shell: Option<PathBuf>,
    /// `cmd.exe`.
    pub cmd: Option<PathBuf>,
    /// `claude`.
    pub claude: Option<PathBuf>,
    /// `codex`.
    pub codex: Option<PathBuf>,
}

/// Konfiguracja (`[ui.terminal]`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TerminalConfig {
    /// Programy profili.
    pub programs: TerminalPrograms,
    /// Katalog startowy (domyślnie profil użytkownika).
    pub default_cwd: PathBuf,
    /// Najwięcej otwartych terminali naraz.
    pub max_sessions: usize,
}

impl Default for TerminalConfig {
    fn default() -> Self {
        Self {
            programs: TerminalPrograms::default(),
            default_cwd: PathBuf::from("."),
            max_sessions: 3,
        }
    }
}

/// Żądanie otwarcia.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OpenRequest {
    /// Profil.
    pub profile: TerminalProfile,
    /// Rozmiar (kolumny × wiersze z xterm.js).
    pub size: PtySize,
    /// Katalog startowy (domyślnie z konfiguracji).
    pub cwd: Option<PathBuf>,
}

/// Dowód gestu użytkownika w UI (kliknięcie/klawisz w panelu terminala). Tworzy go wyłącznie
/// komenda UI w korzeniu kompozycji (`app-*`) przez [`ui_only::user_gesture`]; żaden crate
/// agentek nie zależy od tego modułu (test grafu zależności).
#[derive(Debug)]
pub struct UserGesture {
    _private: (),
}

/// Tylko dla komend UI w korzeniu kompozycji.
pub mod ui_only {
    use super::UserGesture;

    /// Gest użytkownika — wołać wyłącznie w obsłudze komendy UI wywołanej przez użytkownika.
    pub fn user_gesture() -> UserGesture {
        UserGesture { _private: () }
    }
}

/// Odbiorca strumienia (kanał UI, np. `tauri::ipc::Channel`). Nigdy magistrala ani log.
pub trait TerminalSink: Send + Sync {
    /// Wyjście VT procesu.
    fn output(&self, id: TerminalId, chunk: &[u8]);
    /// Proces zakończył się (kod, jeśli znany); po tym nie będzie już wyjścia.
    fn exited(&self, id: TerminalId, code: Option<i32>);
}

/// Stan sesji (bez treści).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TerminalInfo {
    /// Identyfikator.
    pub id: TerminalId,
    /// Profil.
    pub profile: TerminalProfile,
    /// PID procesu głównego.
    pub pid: u32,
    /// Czy proces działa.
    pub alive: bool,
}

/// Błąd terminala.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum TerminalError {
    /// Nieznana sesja.
    #[error("nieznany terminal {0}")]
    NotFound(u64),
    /// Limit otwartych terminali.
    #[error("otwarto już {0} terminale — zamknij któryś")]
    Limit(usize),
    /// Brak programu profilu (CLI niezainstalowane).
    #[error("brak programu dla profilu {0} — zainstaluj CLI albo wskaż ścieżkę w Ustawieniach")]
    ProgramMissing(String),
    /// Paczka wejścia za duża.
    #[error("paczka wejścia większa niż {MAX_INPUT_BYTES} B")]
    InputTooLarge,
    /// Błąd platformy.
    #[error("terminal: {0}")]
    Platform(String),
}

/// Usługa terminali — **tylko dla UI** (komendy `terminal_*`), nigdy dla agentek.
pub trait TerminalService: Send + Sync {
    /// Otwiera terminal; wyjście płynie do `sink` na osobnym wątku.
    fn open(
        &self,
        request: OpenRequest,
        gesture: &UserGesture,
        sink: Arc<dyn TerminalSink>,
    ) -> Result<TerminalId, TerminalError>;
    /// Wejście z klawiatury UI.
    fn input(
        &self,
        id: TerminalId,
        data: &[u8],
        gesture: &UserGesture,
    ) -> Result<(), TerminalError>;
    /// Zmiana rozmiaru.
    fn resize(&self, id: TerminalId, size: PtySize) -> Result<(), TerminalError>;
    /// Zamyka terminal i zabija drzewo procesów (idempotentne dla znanej sesji).
    fn close(&self, id: TerminalId) -> Result<(), TerminalError>;
    /// Otwarte terminale.
    fn list(&self) -> Vec<TerminalInfo>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn types_and_errors() {
        let g = ui_only::user_gesture();
        assert!(format!("{g:?}").contains("UserGesture"));
        assert_eq!(TerminalConfig::default().max_sessions, 3);
        for e in [
            TerminalError::NotFound(1),
            TerminalError::Limit(3),
            TerminalError::ProgramMissing("claude".into()),
            TerminalError::InputTooLarge,
            TerminalError::Platform("x".into()),
        ] {
            assert!(!e.to_string().is_empty());
        }
        let r = OpenRequest {
            profile: TerminalProfile::CodexLogin,
            size: PtySize::default(),
            cwd: None,
        };
        assert_eq!(r.profile, TerminalProfile::CodexLogin);
    }
}
