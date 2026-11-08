//! Typy kontraktu dyktowania: konfiguracja, cel, stan, zdarzenia (bez treści), błędy.

use platform_contract::{WindowId, image_file_name};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Obrazy procesów terminali (Enter wykonuje polecenie — „nowa linia” jest blokowana).
pub const TERMINAL_IMAGES: [&str; 12] = [
    "cmd.exe",
    "powershell.exe",
    "pwsh.exe",
    "windowsterminal.exe",
    "wt.exe",
    "conhost.exe",
    "openconsole.exe",
    "bash.exe",
    "wsl.exe",
    "mintty.exe",
    "putty.exe",
    "alacritty.exe",
];

/// Czy obraz procesu to terminal.
pub fn is_terminal_image(image: &str) -> bool {
    let name = image_file_name(image);
    TERMINAL_IMAGES.contains(&name.as_str())
}

/// Konfiguracja (`[voice.dictation]`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct DictationCfg {
    /// Wielka litera na początku sesji.
    pub capitalize_start: bool,
    /// „nowa linia” w terminalu → spacja (Enter naciska użytkownik).
    pub block_enter_in_terminals: bool,
    /// Ile ms po wpisaniu frazy działa „cofnij to”.
    pub undo_window_ms: u64,
    /// Najdłuższa fraza (znaki) — dłuższa jest odrzucana (halucynacja STT).
    pub max_phrase_chars: usize,
    /// Tryb przełącznika: koniec sesji po tylu ms bez mowy (0 = bez limitu).
    pub idle_stop_ms: u64,
}

impl Default for DictationCfg {
    fn default() -> Self {
        Self {
            capitalize_start: true,
            block_enter_in_terminals: true,
            undo_window_ms: 30_000,
            max_phrase_chars: 2_000,
            idle_stop_ms: 60_000,
        }
    }
}

/// Tryb sesji.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DictationMode {
    /// Mów, trzymając klawisz (sesja = przytrzymanie).
    PushToTalk,
    /// Przełącznik (sesja do „koniec dyktowania” / klawisza / bezczynności).
    Toggle,
}

/// Okno docelowe ustalone w chwili startu.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DictationTarget {
    /// Okno.
    pub window: WindowId,
    /// PID procesu okna.
    pub pid: u32,
    /// Nazwa pliku procesu (np. `notepad.exe`) — do zdarzeń i UI.
    pub app: String,
    /// Terminal (Enter blokowany).
    pub terminal: bool,
}

/// Dlaczego pauza.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum PauseReason {
    /// Na pierwszym planie jest inne okno niż cel.
    FocusChanged,
    /// Użytkownik pisze / klika (fizyczne wejście ma pierwszeństwo).
    UserTyping,
}

/// Faza sesji.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "phase", content = "reason", rename_all = "snake_case")]
pub enum DictationPhase {
    /// Brak sesji.
    Idle,
    /// Wpisuje.
    Active,
    /// Wstrzymana (tekst czeka).
    Paused(PauseReason),
}

/// Dlaczego koniec.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum StopReason {
    /// Użytkownik (UI, klawisz, puszczenie PTT).
    User,
    /// Komenda głosowa „koniec dyktowania”.
    Voice,
    /// Bezczynność.
    Idle,
    /// Okno celu zniknęło.
    TargetGone,
    /// Nowa sesja.
    Restarted,
    /// Anulowanie (kill-switch, „stop”).
    Cancelled,
}

/// Dlaczego odmowa.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RefuseReason {
    /// Brak okna na pierwszym planie.
    NoForeground,
    /// Okno Alfy, Brokera albo procesu nieznanego.
    ProtectedTarget,
    /// Okno administratora (UIPI bez helpera `uiAccess`).
    ElevatedTarget,
    /// Fokus w polu hasła (UIA `IsPassword`) albo nie da się tego wykluczyć.
    PasswordField,
}

/// Stan (UI: pigułka dyktowania).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct DictationStatus {
    /// Faza.
    pub phase: DictationPhase,
    /// Tryb.
    pub mode: DictationMode,
    /// Aplikacja celu.
    pub app: Option<String>,
    /// Znaki czekające na wpisanie.
    pub pending_chars: usize,
    /// Znaki wpisane w sesji.
    pub typed_chars: usize,
    /// Czy „cofnij to” ma co cofnąć.
    pub can_undo: bool,
}

/// Zdarzenia (bez dyktowanego tekstu).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum DictationEvent {
    /// `voice.dictation.started`.
    Started {
        /// Aplikacja celu.
        app: String,
        /// Tryb.
        mode: DictationMode,
    },
    /// `voice.dictation.typed`.
    Typed {
        /// Znaki.
        chars: u32,
    },
    /// `voice.dictation.paused`.
    Paused {
        /// Powód.
        reason: PauseReason,
    },
    /// `voice.dictation.resumed`.
    Resumed,
    /// `voice.dictation.stopped`.
    Stopped {
        /// Powód.
        reason: StopReason,
        /// Znaki niewpisane (porzucone).
        dropped_chars: u32,
    },
    /// `voice.dictation.refused`.
    Refused {
        /// Powód.
        reason: RefuseReason,
    },
    /// `voice.dictation.undone`.
    Undone {
        /// Usunięte znaki (0 — fraza jeszcze niewpisana).
        chars: u32,
    },
    /// `voice.dictation.undo_unavailable`.
    UndoUnavailable,
    /// `voice.dictation.newline_blocked`.
    NewlineBlocked,
}

/// Błędy.
#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "error", content = "detail", rename_all = "snake_case")]
pub enum DictationError {
    /// Odmowa startu.
    #[error("dyktowanie niedostępne: {0:?}")]
    Refused(RefuseReason),
    /// Brak sesji.
    #[error("dyktowanie nie jest włączone")]
    NotActive,
    /// Fraza odrzucona (za długa — prawdopodobnie halucynacja STT).
    #[error("fraza odrzucona: {0}")]
    Rejected(String),
    /// Błąd platformy (wejście, UIA, okna).
    #[error("platforma: {0}")]
    Platform(String),
}
