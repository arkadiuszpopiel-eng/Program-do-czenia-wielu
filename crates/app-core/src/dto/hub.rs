//! DTO Hubu kont i kluczy, uprawnień i urządzeń (odpowiedniki `types-hub.ts`).

use std::fmt;

use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

use super::common::{AutonomyLevel, Iso8601, LocalizedText, Money};

/// Rodzaj usług dostawcy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderKind {
    Chat,
    Stt,
    Tts,
    Multi,
}

/// Status zgodności.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ComplianceStatus {
    Green,
    Gray,
    Forbidden,
    Unverified,
}

/// Uwierzytelnienie.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthKind {
    ApiKey,
    OauthCli,
    None,
}

/// Zgodność protokołu.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompatKind {
    Openai,
    Anthropic,
    Native,
}

/// Wpis katalogu dostawców.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderInfo {
    pub id: String,
    pub display_name: String,
    pub kind: ProviderKind,
    pub auth: AuthKind,
    pub compat: CompatKind,
    pub privacy_tag: String,
    pub jurisdiction: String,
    pub compliance_status: ComplianceStatus,
    pub terms_url: Option<String>,
    pub needs_base_url: bool,
}

/// Stan konta w UI.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AccountState {
    Unconfigured,
    Testing,
    Ok,
    Invalid,
    RateLimited,
    Disabled,
}

/// Rodzaj modelu.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelKind {
    Chat,
    Stt,
    Tts,
    Embeddings,
    Vision,
}

/// Model konta.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelInfo {
    pub id: String,
    pub name: String,
    pub kinds: Vec<ModelKind>,
    pub context_tokens: Option<u64>,
}

/// Przypisania konta.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccountAssignment {
    pub task_classes: Vec<String>,
    pub agents: Vec<String>,
    pub voice_stt: bool,
    pub voice_tts: bool,
}

/// Limit kosztów konta.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccountCostLimit {
    pub enabled: bool,
    pub monthly: Money,
}

/// Konto (bez sekretu).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Account {
    pub id: String,
    pub provider_id: String,
    pub label: String,
    pub state: AccountState,
    pub key_stored: bool,
    pub models: Vec<ModelInfo>,
    pub assignments: AccountAssignment,
    pub cost_limit: AccountCostLimit,
    pub last_tested_at: Option<Iso8601>,
}

/// Sekret z kreatora: zerowany w pamięci, `Debug` nie ujawnia wartości, brak `Serialize`.
#[derive(Clone)]
pub struct SecretInput(Zeroizing<String>);

impl<'de> Deserialize<'de> for SecretInput {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        String::deserialize(d).map(|s| Self(Zeroizing::new(s)))
    }
}

impl SecretInput {
    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for SecretInput {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("SecretInput(***)")
    }
}

impl From<&str> for SecretInput {
    fn from(value: &str) -> Self {
        Self(Zeroizing::new(value.to_owned()))
    }
}

/// Dane kreatora konta. Sekret trafia wyłącznie do Credential Managera.
#[derive(Debug, Clone, Deserialize)]
pub struct AddAccountInput {
    pub provider_id: String,
    pub label: String,
    pub secret: SecretInput,
    pub base_url: Option<String>,
}

/// Raport testu konta.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TestReport {
    pub ok: bool,
    pub latency_ms: Option<u64>,
    pub models: Vec<ModelInfo>,
    pub error: Option<String>,
}

/// Uprawnienia (PLAN §8.3).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PermissionsState {
    pub global: AutonomyLevel,
    pub session: Option<AutonomyLevel>,
    pub hello_enabled: bool,
}

/// Stan intencji Brokera.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BrokerIntentStatus {
    OpenedBroker,
}

/// Wynik intencji Brokera.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BrokerIntentResult {
    pub status: BrokerIntentStatus,
    pub request_id: String,
}

/// Klasa sprzętu.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HwClass {
    Baseline,
    StandardAmd,
    LaptopCuda,
    Strong,
    Unknown,
}

/// Profil głosu A–D.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum VoiceProfileId {
    A,
    B,
    C,
    D,
}

/// Karta graficzna.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GpuView {
    pub vendor: String,
    pub model: String,
    pub vram_mb: u64,
    pub backends: Vec<String>,
}

/// Procesor.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CpuView {
    pub model: String,
    pub cores: u32,
    pub threads: u32,
}

/// Bateria.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct BatteryView {
    pub percent: u8,
    pub on_ac: bool,
}

/// Maszyna.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MachineView {
    pub id: String,
    pub name: String,
    pub os: String,
    pub cpu: CpuView,
    pub ram_mb: u64,
    pub gpus: Vec<GpuView>,
    pub npu: Option<String>,
    pub battery: Option<BatteryView>,
}

/// Rekomendacja profilu.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecommendationView {
    pub hw_class: HwClass,
    pub voice_profile: VoiceProfileId,
    pub llm_backend: String,
    pub tradeoffs: Vec<LocalizedText>,
}

/// Profil urządzenia.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeviceProfile {
    pub machine: MachineView,
    pub recommendation: RecommendationView,
    pub measured_at: Iso8601,
}

/// Urządzenie audio.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AudioDevice {
    pub id: String,
    pub name: String,
    pub default: bool,
}
