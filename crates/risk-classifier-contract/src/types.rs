//! Typy wejścia klasyfikatora: poziomy autonomii, fakty akcji, reguły Jądra.

use std::fmt;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Poziom autonomii (PLAN §8.3). Porządek rosnący: L0 < L1 < L2 < L3 < L4; domyślnie L3.
#[derive(
    Debug,
    Clone,
    Copy,
    Default,
    PartialEq,
    Eq,
    Hash,
    PartialOrd,
    Ord,
    Serialize,
    Deserialize,
    JsonSchema,
)]
pub enum AutonomyLevel {
    /// Podgląd — tylko czyta i podpowiada.
    L0,
    /// Pytaj o wszystko — każda zmiana wymaga „tak”.
    L1,
    /// Pytaj o ryzykowne — usuwanie, egress, instalacja.
    L2,
    /// Bardzo wysoka (domyślny) — pyta przy nieodwracalnych poza zakresem i niezaufanej treści.
    #[default]
    L3,
    /// Maks — tylko twarde blokady Jądra, destrukcja głosem i taint dla egressu.
    L4,
}

impl AutonomyLevel {
    /// Wszystkie poziomy rosnąco.
    pub const ALL: [AutonomyLevel; 5] = [Self::L0, Self::L1, Self::L2, Self::L3, Self::L4];

    /// Nazwa po polsku (do UI i audytu).
    pub fn name_pl(self) -> &'static str {
        match self {
            Self::L0 => "Podgląd",
            Self::L1 => "Pytaj o wszystko",
            Self::L2 => "Pytaj o ryzykowne",
            Self::L3 => "Bardzo wysoka",
            Self::L4 => "Maks",
        }
    }
}

impl fmt::Display for AutonomyLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?} ({})", self.name_pl())
    }
}

/// Klasa ryzyka akcji. Porządek rosnący.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum RiskLevel {
    /// Niskie.
    Low,
    /// Średnie.
    Medium,
    /// Wysokie.
    High,
    /// Krytyczne.
    Critical,
}

impl RiskLevel {
    /// Poziom o jeden wyżej (Critical pozostaje Critical).
    pub fn bumped(self) -> Self {
        match self {
            Self::Low => Self::Medium,
            Self::Medium => Self::High,
            Self::High | Self::Critical => Self::Critical,
        }
    }

    /// Etykieta po polsku (karta zatwierdzenia: kolor zawsze z tekstem).
    pub fn label_pl(self) -> &'static str {
        match self {
            Self::Low => "niskie",
            Self::Medium => "średnie",
            Self::High => "wysokie",
            Self::Critical => "krytyczne",
        }
    }
}

/// Odwracalność z manifestu narzędzia (`reversible: yes|scoped|no`, PLAN §8.7).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Reversibility {
    /// W pełni odwracalna (dziennik cofania).
    Yes,
    /// Odwracalna w zakresie (snapshot zakresu przed wykonaniem).
    Scoped,
    /// Nieodwracalna.
    No,
}

/// Relacja celu akcji do zakresu sesji.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ScopeRelation {
    /// W profilu użytkownika / zakresie sesji.
    InScope,
    /// We wskazanej (dozwolonej) aplikacji.
    AllowedApp,
    /// Poza zakresem.
    Outside,
}

/// Destrukcyjność akcji.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum Destructiveness {
    /// Nic nie niszczy.
    None,
    /// Niszczy odzyskiwalnie (Kosz, pre-image).
    Recoverable,
    /// Niszczy trwale.
    Permanent,
}

/// Pewność rozpoznania mowy w promilach (0–1000); liczby całkowite = determinizm.
#[derive(
    Debug,
    Clone,
    Copy,
    Default,
    PartialEq,
    Eq,
    Hash,
    PartialOrd,
    Ord,
    Serialize,
    Deserialize,
    JsonSchema,
)]
#[serde(try_from = "u16", into = "u16")]
pub struct SttConfidence(u16);

impl SttConfidence {
    /// Z promili; wartości > 1000 są przycinane do 1000.
    pub fn from_permille(permille: u16) -> Self {
        Self(permille.min(1000))
    }

    /// Z ułamka 0,0–1,0 (zaokrąglenie do promila; NaN i wartości ujemne → 0).
    pub fn from_ratio(ratio: f32) -> Self {
        let scaled = (ratio * 1000.0).round();
        let clamped = if scaled.is_nan() || scaled <= 0.0 {
            0
        } else if scaled >= 1000.0 {
            1000
        } else {
            // Mieści się w zakresie 1..=999, więc konwersja jest dokładna.
            scaled as u16
        };
        Self(clamped)
    }

    /// Wartość w promilach.
    pub fn permille(self) -> u16 {
        self.0
    }
}

impl TryFrom<u16> for SttConfidence {
    type Error = String;

    fn try_from(value: u16) -> Result<Self, Self::Error> {
        if value > 1000 {
            return Err(format!("pewność STT {value}‰ > 1000‰"));
        }
        Ok(Self(value))
    }
}

impl From<SttConfidence> for u16 {
    fn from(value: SttConfidence) -> Self {
        value.0
    }
}

/// Źródło polecenia (kto „zlecił” akcję).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "origin", rename_all = "snake_case")]
pub enum CommandOrigin {
    /// Użytkownik tekstem (composer, UI).
    UserText,
    /// Użytkownik głosem — z pewnością STT i wynikiem weryfikacji mówcy.
    UserVoice {
        /// Pewność rozpoznania mowy.
        confidence: SttConfidence,
        /// Czy `voice-speaker` potwierdził właściciela (do F5 zawsze `false`).
        speaker_verified: bool,
    },
    /// Agentka z własnej inicjatywy (plan, krok).
    Agent,
    /// Polecenie pochodzi z niezaufanej treści (strona, mail, plik, OCR, dźwięk z TV).
    UntrustedContent,
}

impl CommandOrigin {
    /// Czy polecenie zlecono głosem.
    pub fn is_voice(&self) -> bool {
        matches!(self, Self::UserVoice { .. })
    }

    /// Rodzaj źródła bez danych (do dopasowania planu).
    pub fn kind(&self) -> &'static str {
        match self {
            Self::UserText => "user_text",
            Self::UserVoice { .. } => "user_voice",
            Self::Agent => "agent",
            Self::UntrustedContent => "untrusted_content",
        }
    }
}

/// Klasa akcji (odpowiada rodzinie tokenu zdolności).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ActionClass {
    /// `fs.read`.
    Read,
    /// `fs.write` (zapis, przeniesienie, usunięcie).
    Write,
    /// `shell.exec`.
    Shell,
    /// `gui.control`.
    GuiControl,
    /// `net.egress`.
    Egress,
    /// `secrets.read`.
    SecretsRead,
    /// `system.admin`.
    Admin,
}

/// Twarde reguły Jądra — obowiązują na każdym poziomie, także L4 (THREAT_MODEL §7).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum KernelRule {
    /// Wyłączenie lub czyszczenie audytu.
    AuditDisable,
    /// Wyłączenie watchdoga, kill-switcha albo usługi Brokera.
    KillSwitchDisable,
    /// Formatowanie dysku systemowego.
    SystemDiskFormat,
    /// Usuwanie `%SystemRoot%` lub korzenia dysku systemowego.
    SystemRootDeletion,
    /// Modyfikacja bootloadera.
    BootloaderModification,
    /// Zmiana polityk Jądra z pominięciem Broker-UI.
    KernelPolicyChange,
    /// Agentka próbuje podnieść własny poziom autonomii.
    SelfEscalation,
    /// `gui.control` wobec procesów Alfy, Brokera, Broker-UI, watchdoga, helpera.
    GuiControlOfKernelProcess,
    /// Deny-lista poświadczeń (tokeny CLI, profile przeglądarek, Credential Manager).
    CredentialDenylist,
    /// Deny-lista webowych UI dostawców (claude.ai, chatgpt.com, …).
    ProviderWebUi,
    /// Polecenie powłoki nieczytelne dla reguł (zakodowane/zaciemnione) — nie da się go sprawdzić.
    OpaqueShellCommand,
}

impl KernelRule {
    /// Opis po polsku (karta, audyt).
    pub fn description_pl(self) -> &'static str {
        match self {
            Self::AuditDisable => "wyłączenie lub czyszczenie audytu jest zablokowane przez Jądro",
            Self::KillSwitchDisable => {
                "wyłączenie watchdoga, kill-switcha lub Brokera jest zablokowane przez Jądro"
            }
            Self::SystemDiskFormat => "formatowanie dysku systemowego jest zablokowane przez Jądro",
            Self::SystemRootDeletion => "usuwanie katalogu Windows jest zablokowane przez Jądro",
            Self::BootloaderModification => "modyfikacja bootloadera jest zablokowana przez Jądro",
            Self::KernelPolicyChange => {
                "polityki Jądra zmienia wyłącznie właściciel w oknie Brokera"
            }
            Self::SelfEscalation => "agentka nie może podnieść własnego poziomu autonomii",
            Self::GuiControlOfKernelProcess => {
                "sterowanie oknami Alfy, Brokera i watchdoga jest zablokowane przez Jądro"
            }
            Self::CredentialDenylist => {
                "dostęp do poświadczeń (CLI, przeglądarki, Credential Manager) jest zablokowany"
            }
            Self::ProviderWebUi => "automatyzacja webowych UI dostawców modeli jest zablokowana",
            Self::OpaqueShellCommand => {
                "polecenie zakodowane lub zaciemnione — Jądro nie może go sprawdzić"
            }
        }
    }
}
