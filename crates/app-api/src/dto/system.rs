//! DTO ustawień, stanu systemu, startu aplikacji i układu okna (odpowiedniki `types-system.ts`).

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::agents::VoiceSpeaker;
use super::common::{Iso8601, Locale, LocalizedText, ModelProfile, px};

/// Wartość ustawienia (`boolean | number | string`); liczba zachowuje postać całkowitą/ułamkową.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum SettingValue {
    Bool(bool),
    Number(serde_json::Number),
    Text(String),
}

impl SettingValue {
    pub fn from_json(value: &serde_json::Value) -> Option<Self> {
        match value {
            serde_json::Value::Bool(b) => Some(Self::Bool(*b)),
            serde_json::Value::Number(n) => Some(Self::Number(n.clone())),
            serde_json::Value::String(s) => Some(Self::Text(s.clone())),
            _ => None,
        }
    }

    pub fn to_json(&self) -> serde_json::Value {
        match self {
            Self::Bool(b) => serde_json::Value::Bool(*b),
            Self::Number(n) => serde_json::Value::Number(n.clone()),
            Self::Text(s) => serde_json::Value::String(s.clone()),
        }
    }
}

/// Zakres ustawienia.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SettingScope {
    Global,
    Session,
    Agent,
    Machine,
}

/// Opcja listy wyboru.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SelectOption {
    pub value: String,
    pub label: LocalizedText,
}

/// Kontrolka ustawienia.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SettingControl {
    Toggle,
    Select {
        options: Vec<SelectOption>,
    },
    Number {
        min: f64,
        max: f64,
        step: f64,
        unit: Option<String>,
    },
    Text,
}

/// Definicja ustawienia.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SettingDef {
    pub key: String,
    pub label: LocalizedText,
    pub description: LocalizedText,
    pub control: SettingControl,
    pub default: SettingValue,
    pub scope: SettingScope,
}

/// Strona z własnym widokiem.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SettingsCustomPage {
    Providers,
    Costs,
    Shortcuts,
    Transfer,
    Permissions,
    Devices,
    Voice,
    /// Pamięć: porządkowanie (Strażniczka), Inspektor na pełną szerokość.
    Memory,
    /// Zadania w tle: wyzwalacze (czas, zdarzenia, ręczne) i dziennik uruchomień.
    Triggers,
    /// Reguły Marszałka: propozycja → podgląd zawężenia → zatwierdź / cofnij.
    Marshal,
    /// Umiejętności: biblioteka, propozycje z diffem i hashem, kwarantanna, eksport/import.
    Skills,
    /// Kreator agentek: rozmowa/formularz → podgląd persony → test na sucho → zapis.
    Builder,
    /// Computer use: podgląd pulpitu „zawsze zezwalaj" (przez Brokera), strażnik okien.
    Computer,
    /// Zdrowie systemu: moduły, incydenty i naprawy Diagnosty, Ulepszacz, wyniki evali.
    Health,
    /// Aktualizacje: stan, pobieranie, „Uruchom ponownie”, „Przywróć poprzednią wersję”.
    Updates,
    /// O programie: wersja, kanał, data kompilacji, licencje zależności.
    About,
}

/// Strona ustawień (drzewo §15).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SettingsPageDef {
    pub id: String,
    pub label: LocalizedText,
    pub wave: u32,
    pub custom: Option<SettingsCustomPage>,
    pub settings: Vec<SettingDef>,
    pub upcoming: Vec<LocalizedText>,
}

/// Limit zapytań u dostawcy.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RateLimitInfo {
    pub provider: String,
    pub resets_at: Iso8601,
}

/// Stan mikrofonu (uprawnienie/obecność).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MicAvailability {
    Ok,
    Denied,
    Missing,
}

/// Dysk.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiskInfo {
    pub free_bytes: u64,
    pub low: bool,
}

/// Stan systemu (PLAN §14.4).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SystemStatus {
    pub online: bool,
    pub queued_messages: u64,
    pub rate_limit: Option<RateLimitInfo>,
    pub keys_configured: bool,
    pub profile: ModelProfile,
    pub mic: MicAvailability,
    pub disk: DiskInfo,
}

/// Panel prawy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PanelId {
    Agents,
    Timeline,
    Files,
    Memory,
    Screen,
    Voice,
    /// Zadania (DAG schedulera, sterowanie, anuluj, ponów).
    Tasks,
}

/// Otwarte panele sesji.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionPanels {
    pub left_open: bool,
    pub right_open: bool,
    pub right_tab: PanelId,
}

/// Układ okna (per maszyna).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LayoutPrefs {
    #[serde(deserialize_with = "px::deserialize")]
    pub left_width: u32,
    #[serde(deserialize_with = "px::deserialize")]
    pub right_width: u32,
    pub left_collapsed: bool,
    pub sessions: BTreeMap<String, SessionPanels>,
}

/// Dane startowe UI.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppBootstrap {
    pub app_version: String,
    pub locale: Locale,
    pub onboarding_done: bool,
    pub machine_name: String,
    pub settings: BTreeMap<String, SettingValue>,
    pub layout: Option<LayoutPrefs>,
    pub active_session_id: Option<String>,
    pub shortcut_overrides: BTreeMap<String, String>,
}

/// Wynik Szybkiego pytania.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuickAskResult {
    pub session_id: String,
    pub user_turn_id: String,
    pub assistant_turn_id: Option<String>,
}

/// Stan mikrofonu w pigułce.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MicState {
    Off,
    Listening,
    Hearing,
    Processing,
    Speaking,
    Muted,
    Dnd,
}

/// Pigułka głosowa.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VoicePillState {
    pub agent: String,
    pub mic: MicState,
    pub level: f64,
    /// Kto mówi teraz.
    #[serde(default)]
    pub speaker: VoiceSpeaker,
    /// Transkrypt częściowy wypowiedzi użytkownika (szary w UI).
    #[serde(default)]
    pub partial: Option<String>,
}
