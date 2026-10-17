//! DTO Hubu kont i kluczy, uprawnień i urządzeń (odpowiedniki `types-hub.ts`).

use std::fmt;

use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

use super::common::{AutonomyLevel, Iso8601, LocalizedText, Money};

/// Rodzaj usług dostawcy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ProviderKind {
    Chat,
    Stt,
    Tts,
    Multi,
}

/// Status zgodności.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ComplianceStatus {
    Green,
    Gray,
    Forbidden,
    Unverified,
}

/// Uwierzytelnienie.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum AuthKind {
    ApiKey,
    OauthCli,
    None,
}

/// Zgodność protokołu.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum CompatKind {
    Openai,
    Anthropic,
    Native,
}

/// Wpis katalogu dostawców.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, schemars::JsonSchema)]
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ModelKind {
    Chat,
    Stt,
    Tts,
    Embeddings,
    Vision,
}

/// Wynik importu jednej zmiennej środowiskowej (tylko nazwa zmiennej — nigdy wartość).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct EnvImportResult {
    pub provider: String,
    pub provider_name: String,
    /// Nazwa zmiennej, np. `OPENAI_API_KEY`.
    pub var: String,
    pub result: EnvImportKind,
    /// Konto dodane albo już istniejące z tym kluczem.
    pub account_id: Option<String>,
}

/// Co stało się ze zmienną.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum EnvImportKind {
    Imported,
    AlreadyPresent,
    NotSet,
}

/// Model konta.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ModelInfo {
    pub id: String,
    pub name: String,
    pub kinds: Vec<ModelKind>,
    pub context_tokens: Option<u64>,
    /// Przeznaczenie rozpoznane po nazwie: `chat`, `reasoning`, `embedding`, `speech`,
    /// `transcription`, `realtime`, `image`, `video`, `moderation`, `other`.
    pub purpose: String,
    /// Wielkość: `standard`, `small`, `tiny`, `premium`.
    pub tier: String,
    /// Przypięta migawka z datą w nazwie (`…-2024-08-06`).
    pub snapshot: bool,
}

/// Przypisania konta.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct AccountAssignment {
    pub task_classes: Vec<String>,
    pub agents: Vec<String>,
    pub voice_stt: bool,
    pub voice_tts: bool,
}

/// Limit kosztów konta.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct AccountCostLimit {
    pub enabled: bool,
    pub monthly: Money,
}

/// Konto (bez sekretu).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
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
    /// Kiedy zapisano obecny klucz (dodanie konta albo ostatnia wymiana) — wiek klucza
    /// i przypomnienie o wymianie liczy interfejs.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub key_saved_at: Option<Iso8601>,
    /// Wynik ostatniego testu — lista kont pokazuje powód, nie tylko „klucz odrzucony”.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_test: Option<AccountLastTest>,
    /// Kolejki modeli: klasa zadań (`chat`, `code`, …) → modele w kolejności prób. Brak klasy =
    /// wybór automatyczny.
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub queues: std::collections::BTreeMap<String, Vec<String>>,
    /// Model, którego Alfa użyje do rozmowy bez kolejki (wybór automatyczny).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auto_model: Option<String>,
}

/// Ostatni test konta w skrócie (te same pola co w [`TestReport`], bez listy modeli).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct AccountLastTest {
    pub ok: bool,
    /// `ok`, `invalid_key`, `restricted`, `no_credit`, `rate_limited`, `timeout`, `network`, `unsupported`.
    pub reason: String,
    pub latency_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_code: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

/// Sekret z kreatora: zerowany w pamięci, `Debug` nie ujawnia wartości, brak `Serialize`.
#[derive(Clone)]
pub struct SecretInput(Zeroizing<String>);

impl<'de> Deserialize<'de> for SecretInput {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        String::deserialize(d).map(|s| Self(Zeroizing::new(s)))
    }
}

impl schemars::JsonSchema for SecretInput {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "SecretInput".into()
    }

    fn json_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
        // Na łączu to zwykły napis; wartość nigdy nie wraca z rdzenia.
        schemars::json_schema!({ "type": "string", "writeOnly": true })
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
#[derive(Debug, Clone, Deserialize, schemars::JsonSchema)]
pub struct AddAccountInput {
    pub provider_id: String,
    pub label: String,
    pub secret: SecretInput,
    pub base_url: Option<String>,
}

/// Raport testu konta.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct TestReport {
    pub ok: bool,
    pub latency_ms: Option<u64>,
    pub models: Vec<ModelInfo>,
    pub error: Option<String>,
    /// Rodzaj wyniku, gdy nie jest zwykłym sukcesem: `invalid_key`, `restricted` (klucz działa,
    /// ale bez prawa do listy modeli), `no_credit`, `rate_limited`, `timeout`, `network`,
    /// `unsupported`. Interfejs dobiera do niego wyjaśnienie i dalszy krok.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// Kod HTTP odpowiedzi dostawcy.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<u16>,
    /// Kod błędu dostawcy (np. `invalid_api_key`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_code: Option<String>,
    /// Komunikat dostawcy w oryginale (bez sekretów).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

/// Uprawnienia (PLAN §8.3).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct PermissionsState {
    pub global: AutonomyLevel,
    pub session: Option<AutonomyLevel>,
    pub hello_enabled: bool,
}

/// Stan intencji Brokera: `opened_broker` — prośba czeka w oknie Brokera; `applied` — zmiana
/// niewymagająca zatwierdzenia (obniżenie poziomu) została zastosowana od razu.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum BrokerIntentStatus {
    OpenedBroker,
    Applied,
}

/// Wynik intencji Brokera.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct BrokerIntentResult {
    pub status: BrokerIntentStatus,
    pub request_id: String,
}

/// Klasa sprzętu.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum HwClass {
    Baseline,
    StandardAmd,
    LaptopCuda,
    Strong,
    /// Tylko karta zintegrowana z procesorem.
    Integrated,
    Unknown,
}

/// Profil głosu A–D.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, schemars::JsonSchema)]
pub enum VoiceProfileId {
    A,
    B,
    C,
    D,
}

/// Karta graficzna.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct GpuView {
    pub vendor: String,
    pub model: String,
    pub vram_mb: u64,
    pub backends: Vec<String>,
    /// Rola na maszynie z kilkoma kartami: `heavy` (modele główne), `light` (lekkie zadania
    /// na karcie zintegrowanej), `spare` (nieużywana przez Alfę).
    pub role: String,
    /// Karta zintegrowana (pamięć współdzielona z RAM).
    pub integrated: bool,
    /// Pamięć współdzielona z systemem (MB).
    pub shared_mb: u64,
}

/// Procesor.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct CpuView {
    pub model: String,
    pub cores: u32,
    pub threads: u32,
}

/// Bateria.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct BatteryView {
    pub percent: u8,
    pub on_ac: bool,
}

/// Maszyna.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct RecommendationView {
    pub hw_class: HwClass,
    pub voice_profile: VoiceProfileId,
    pub llm_backend: String,
    pub tradeoffs: Vec<LocalizedText>,
}

/// Profil urządzenia.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct DeviceProfile {
    pub machine: MachineView,
    pub recommendation: RecommendationView,
    pub measured_at: Iso8601,
}

/// Urządzenie audio.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct AudioDevice {
    pub id: String,
    pub name: String,
    pub default: bool,
}

/// Model lokalny z manifestu `providers-local` (onboarding, Ustawienia → Modele lokalne).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct LocalModelInfo {
    pub id: String,
    pub name: String,
    pub size_bytes: u64,
    pub installed: bool,
    pub default: bool,
    pub downloading: bool,
}

/// Stan pobierania modelu lokalnego (zdarzenie `LocalModelProgress`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum LocalDownloadState {
    Downloading,
    Done,
    Failed,
    Cancelled,
}
