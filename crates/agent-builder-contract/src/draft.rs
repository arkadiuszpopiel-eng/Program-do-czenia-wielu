//! Szkic agentki (formularz albo wynik rozmowy — pola opcjonalne, braki zamieniane na pytania)
//! i zwalidowany manifest (persona + rola + głos v0 + limity), zatwierdzenie zapisu, scenariusz
//! testu na sucho.

use agent_runtime_contract::RunBudget;
use personas_contract::{NameForms, Persona, Role};
use risk_classifier_contract::AutonomyLevel;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use voice_tts_contract::VoicePreset;

/// Głos v0: mówczyni bazowa wbudowanego silnika + wysokość i tempo (bez kluczy, bez klonów).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct VoiceDraft {
    /// Mówczyni bazowa (`pl-f1`, `pl-f2`, zapas Piper).
    pub base: String,
    /// Wysokość (0,7–1,4).
    pub pitch: f32,
    /// Tempo (0,6–1,6).
    pub rate: f32,
    /// Postrzegany wiek (18–25).
    pub perceived_age: u8,
    /// Barwa (opis).
    pub timbre: String,
    /// Prompt voice design (bez „girl/cute/child”).
    pub design_prompt: String,
}

/// Rola agentki (zestaw zadań, narzędzi i uprawnień).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RoleDraft {
    /// Identyfikator roli (`[a-z][a-z0-9-]{1,31}`).
    pub id: String,
    /// Nazwa (forma żeńska).
    pub name: String,
    /// Zadania (opis dla UI).
    pub description: String,
    /// Prompt roli (rodzaj żeński).
    pub prompt: String,
    /// Klasa zadań dla routera (`conversation`, `code`, `research`…).
    pub model_policy: String,
    /// Grupy narzędzi (⊆ dozwolone przez politykę Kreatora).
    pub tools: Vec<String>,
    /// Tylko odczyt.
    #[serde(default)]
    pub read_only: bool,
    /// Praca na niezaufanych źródłach w izolacji.
    #[serde(default)]
    pub untrusted_isolated: bool,
    /// Tworzy wyniki weryfikowane przez Krytyczkę.
    #[serde(default)]
    pub author: bool,
}

/// Limity agentki.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct LimitsDraft {
    /// Poziom autonomii (≤ sufit polityki; `None` = sufit).
    #[serde(default)]
    pub autonomy: Option<AutonomyLevel>,
    /// Budżet przebiegu (≤ sufit polityki; `None` = domyślny).
    #[serde(default)]
    pub budget: Option<RunBudget>,
    /// Zakresy zapisu (`%USERPROFILE%\Pobrane\**`), wyłącznie w profilu użytkownika.
    #[serde(default)]
    pub fs_write: Vec<String>,
    /// Zakres pamięci (`agent` albo `session`).
    #[serde(default)]
    pub memory_scope: Option<String>,
    /// Retencja pamięci (dni, 1–365).
    #[serde(default)]
    pub retain_days: Option<u32>,
    /// Wyzwalacze (cron, 5 pól) — dane; aktywacja osobno w module wyzwalaczy.
    #[serde(default)]
    pub triggers: Vec<String>,
}

/// Szkic agentki.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct AgentDraft {
    /// Identyfikator persony (domyślnie z imienia).
    #[serde(default)]
    pub id: Option<String>,
    /// Imię.
    #[serde(default)]
    pub name: Option<String>,
    /// Odmiana imienia (domyślnie wyliczona dla imion żeńskich na „-a”).
    #[serde(default)]
    pub forms: Option<NameForms>,
    /// Glif (domyślnie pierwsza litera imienia).
    #[serde(default)]
    pub glyph: Option<char>,
    /// Token koloru z palety Kreatora.
    #[serde(default)]
    pub color: Option<String>,
    /// Charakter.
    #[serde(default)]
    pub character: Option<String>,
    /// Głos v0.
    #[serde(default)]
    pub voice: Option<VoiceDraft>,
    /// Rola.
    #[serde(default)]
    pub role: Option<RoleDraft>,
    /// Limity.
    #[serde(default)]
    pub limits: LimitsDraft,
    /// Umiejętności (identyfikatory z biblioteki `skills`).
    #[serde(default)]
    pub skills: Vec<String>,
}

/// Limity po walidacji.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct AgentLimits {
    /// Poziom autonomii (≤ sufit; stosowany w Brokerze wyłącznie jako obniżenie).
    pub autonomy: AutonomyLevel,
    /// Budżet przebiegu.
    pub budget: RunBudget,
    /// Zakresy zapisu.
    pub fs_write: Vec<String>,
    /// Zakres pamięci.
    pub memory_scope: String,
    /// Retencja pamięci (dni).
    pub retain_days: u32,
    /// Wyzwalacze (dane).
    pub triggers: Vec<String>,
}

/// Zwalidowany manifest agentki (PLAN §9.5).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct AgentManifest {
    /// Persona (tożsamość, odmiana imienia, kolor, biblia głosu).
    pub persona: Persona,
    /// Rola.
    pub role: Role,
    /// Głos v0.
    pub voice: VoicePreset,
    /// Limity.
    pub limits: AgentLimits,
    /// Umiejętności.
    pub skills: Vec<String>,
}

/// Kanał zatwierdzenia zapisu (głos nie wystarcza — nowa agentka to zmiana R1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum BuilderApprovalOrigin {
    /// Przycisk „Zapisz” w Kreatorze.
    Ui,
    /// Polecenie tekstowe właściciela.
    Text,
    /// Polecenie głosowe (odrzucane przy zapisie).
    Voice,
}

/// Zatwierdzenie zapisu: kanał + hash przejrzanego manifestu.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct BuilderApproval {
    /// Kanał.
    pub origin: BuilderApprovalOrigin,
    /// Hash manifestu z podglądu.
    pub reviewed_hash: String,
}

/// Oczekiwany wynik kroku testu na sucho.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DryExpect {
    /// Agentka może to zrobić sama.
    #[default]
    Allowed,
    /// Zapyta właściciela (Broker-UI).
    Ask,
    /// Odmowa (poza rolą lub zakresem).
    Denied,
}

/// Krok scenariusza.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct DryStep {
    /// Narzędzie.
    pub tool: String,
    /// Argumenty.
    #[serde(default)]
    pub args: serde_json::Value,
    /// Oczekiwanie.
    #[serde(default)]
    pub expect: DryExpect,
}

/// Skryptowany przebieg testu na sucho (bez modelu, bez skutków).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct DryScenario {
    /// Kroki.
    pub steps: Vec<DryStep>,
}

/// Propozycja szkicu z rozmowy: szkic + pytania o braki.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct DraftProposal {
    /// Szkic.
    pub draft: AgentDraft,
    /// Pytania (PL) o brakujące albo niejasne pola.
    pub questions: Vec<String>,
}
