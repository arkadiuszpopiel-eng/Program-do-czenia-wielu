//! DTO panelu „Ekran" (computer use: co agentka widzi i robi, przejęcie sterowania) i wbudowanego
//! terminala (`ui-terminal`) — odpowiedniki `types-work.ts`.

use serde::{Deserialize, Serialize};

use super::common::{Iso8601, LocalizedText};

/// Stan akcji GUI agentki.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum GuiActionStatus {
    Running,
    Ok,
    Denied,
    Failed,
    Cancelled,
}

/// Kto steruje teraz (wskaźnik w pasku tytułu).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct GuiControl {
    pub session_id: String,
    pub agent: String,
    pub tool: String,
    pub since: Iso8601,
}

/// Akcja GUI agentki (bez treści wpisywanej — tylko rodzaj, cel i wynik).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct GuiAction {
    pub id: u64,
    pub at: Iso8601,
    pub session_id: String,
    pub agent: String,
    pub tool: String,
    pub title: String,
    pub target: Option<String>,
    pub status: GuiActionStatus,
    pub summary: String,
    pub duration_ms: Option<u64>,
}

/// Metadane ostatniego zrzutu (piksele — tylko `gui_screenshot`, nigdy w zdarzeniach).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct GuiShotInfo {
    pub at: Iso8601,
    pub session_id: String,
    pub agent: String,
    pub width: u32,
    pub height: u32,
    pub masked: u32,
    pub black_frame: bool,
}

/// Stan panelu „Ekran".
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct GuiStatus {
    pub available: bool,
    pub reason: Option<LocalizedText>,
    pub control: Option<GuiControl>,
    pub taken_over: bool,
    pub actions: Vec<GuiAction>,
    pub screenshot: Option<GuiShotInfo>,
}

/// Ostatni zrzut agentki (zamaskowany w porcie: okna Alfy/Brokera, deny-lista, pola haseł).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct GuiScreenshot {
    pub info: GuiShotInfo,
    pub data_url: String,
}

/// Profil terminala.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum TerminalProfileId {
    Shell,
    Cmd,
    ClaudeLogin,
    CodexLogin,
    AgyLogin,
}

/// Otwarty terminal (bez treści).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct TerminalSession {
    pub id: u64,
    pub profile: TerminalProfileId,
    pub pid: u32,
    pub alive: bool,
}

/// Ramka strumienia terminala — wyłącznie kanał `tauri::ipc::Channel` otwarcia (nigdy zdarzenia).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TerminalFrame {
    Output { data_b64: String },
    Exit { code: Option<i32> },
}
