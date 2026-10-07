//! Model umiejętności: przepis (manifest), źródło i zaufanie, stan w bibliotece, zatwierdzenie
//! właściciela (z hashem przejrzanej wersji — podmiana po przeglądzie jest odrzucana).

use std::fmt;

use agent_runtime_contract::RunBudget;
use schemars::JsonSchema;
use semver::Version;
use serde::{Deserialize, Serialize};

/// Identyfikator umiejętności: `[a-z][a-z0-9-]{1,63}`, bez `-` na końcu.
#[derive(
    Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(transparent)]
pub struct SkillId(pub String);

impl SkillId {
    /// Identyfikator (bez walidacji; patrz [`SkillId::is_valid`]).
    pub fn new(v: impl Into<String>) -> Self {
        Self(v.into())
    }

    /// Widok tekstowy.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Czy format poprawny.
    pub fn is_valid(&self) -> bool {
        let id = self.0.as_str();
        id.chars().next().is_some_and(|c| c.is_ascii_lowercase())
            && (2..=64).contains(&id.len())
            && id
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
            && !id.ends_with('-')
    }
}

impl fmt::Display for SkillId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Przykład użycia (dla modelu i UI).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct SkillExample {
    /// Prośba właściciela.
    pub request: String,
    /// Parametry, które z niej wynikają.
    #[serde(default)]
    pub params: serde_json::Value,
}

/// Test akceptacyjny: parametry → cel przebiegu zawiera teksty (albo walidacja ma odrzucić).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct AcceptanceTest {
    /// Nazwa.
    pub name: String,
    /// Parametry.
    #[serde(default)]
    pub params: serde_json::Value,
    /// Teksty, które musi zawierać cel po wstawieniu parametrów.
    #[serde(default)]
    pub expect_in_goal: Vec<String>,
    /// Oczekiwane odrzucenie parametrów (test negatywny).
    #[serde(default)]
    pub expect_rejected: bool,
}

/// Umiejętność — nazwany, wersjonowany przepis (PLAN §9.5: playbook + narzędzia).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Skill {
    /// Identyfikator.
    pub id: SkillId,
    /// Wersja (aktualizacja = wyższa wersja, znów po zatwierdzeniu).
    #[schemars(with = "String")]
    pub version: Version,
    /// Nazwa (PL).
    pub name: String,
    /// Opis: co robi, kiedy jej użyć.
    pub description: String,
    /// Słowa kluczowe do wyszukiwania.
    #[serde(default)]
    pub keywords: Vec<String>,
    /// Wymagane narzędzia (nazwy z rejestru narzędzi).
    #[serde(default)]
    pub required_tools: Vec<String>,
    /// Wymagane rodziny zdolności (`fs.read`, `fs.write`…) — muszą zgadzać się z manifestami narzędzi.
    #[serde(default)]
    pub required_capabilities: Vec<String>,
    /// JSON Schema parametrów (obiekt; podzbiór: typy proste, `enum`, `required`, `additionalProperties: false`).
    pub parameters: serde_json::Value,
    /// Szablon polecenia dla agentki (`{{parametr}}`).
    pub prompt: String,
    /// Kroki (playbook).
    #[serde(default)]
    pub steps: Vec<String>,
    /// Przykłady.
    #[serde(default)]
    pub examples: Vec<SkillExample>,
    /// Testy akceptacyjne (≥ 1).
    pub acceptance: Vec<AcceptanceTest>,
    /// Sufit budżetu przebiegu umiejętności (∩ budżet wywołującej).
    #[serde(default)]
    pub budget: Option<RunBudget>,
}

/// Skąd pochodzi paczka importu.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ImportOrigin {
    /// Własna paczka `.alfa` (przeniesienie między maszynami właściciela).
    OwnPackage,
    /// Plik z zewnątrz (pobrany, przysłany) — treść niezaufana.
    External,
}

/// Źródło umiejętności.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "source", rename_all = "snake_case")]
pub enum SkillSource {
    /// Pamięć proceduralna (konsolidacja Strażniczki pamięci).
    Memory {
        /// Wpis źródłowy (`zakres#id`).
        entry: String,
        /// Wpis z treści zaufanej (proweniencja pamięci).
        trusted: bool,
    },
    /// Właściciel (formularz, rozmowa).
    User,
    /// Import paczki.
    Import {
        /// Pochodzenie paczki.
        origin: ImportOrigin,
    },
}

impl SkillSource {
    /// Czy treść pochodzi z niezaufanego źródła (→ kwarantanna).
    pub fn is_untrusted(&self) -> bool {
        matches!(
            self,
            Self::Memory { trusted: false, .. }
                | Self::Import {
                    origin: ImportOrigin::External
                }
        )
    }
}

/// Stan wersji umiejętności w bibliotece.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SkillState {
    /// Czeka na zatwierdzenie właściciela.
    Proposed,
    /// Kwarantanna (treść niezaufana albo podejrzana) — nie do uruchomienia ani wyszukania.
    Quarantined,
    /// Zainstalowana (aktywna wersja).
    Installed,
    /// Odrzucona przez właściciela.
    Rejected,
    /// Wyłączona przez właściciela.
    Disabled,
    /// Zastąpiona nowszą zainstalowaną wersją (historia, cofnięcie).
    Superseded,
}

/// Kanał zatwierdzenia (wyłącznie właściciel; agentka nie ma tu wariantu).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalOrigin {
    /// Kliknięcie w UI (karta umiejętności).
    Ui,
    /// Polecenie tekstowe właściciela.
    Text,
    /// Polecenie głosowe właściciela (nie wystarcza do zwolnienia z kwarantanny).
    Voice,
}

/// Zatwierdzenie właściciela: kanał + hash wersji, którą przejrzał.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct OwnerApproval {
    /// Kanał.
    pub origin: ApprovalOrigin,
    /// Hash przejrzanej treści (musi równać się hashowi wersji).
    pub reviewed_hash: String,
}

/// Wersja umiejętności w bibliotece.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct SkillRecord {
    /// Przepis.
    pub skill: Skill,
    /// Hash treści (SHA-256 kanonicznego JSON-a, hex).
    pub hash: String,
    /// Źródło.
    pub source: SkillSource,
    /// Stan.
    pub state: SkillState,
    /// Uwagi skanera treści (powody kwarantanny, ostrzeżenia dla właściciela).
    #[serde(default)]
    pub findings: Vec<String>,
    /// Zatwierdzenie (instalacja / zwolnienie z kwarantanny).
    #[serde(default)]
    pub approval: Option<OwnerApproval>,
    /// Kiedy zgłoszono (ms).
    pub proposed_at_ms: u64,
    /// Kiedy rozstrzygnięto (ms).
    #[serde(default)]
    pub decided_at_ms: Option<u64>,
}
