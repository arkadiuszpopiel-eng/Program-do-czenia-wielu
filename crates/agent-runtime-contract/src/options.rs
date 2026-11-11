//! Opcje przebiegu v1 (PLAN §9.1–9.2, §9.6): obsada do delegacji i Krytyczki, koperta uprawnień,
//! pochodzenie (przebieg-rodzic, głębokość), taint odziedziczony. Zapisywane w checkpoincie —
//! wznowienie po restarcie zachowuje delegację i weryfikację.

use std::collections::BTreeMap;

use core_bus_contract::RunId;
use personas_contract::{Cast, Persona, PersonaId, Role, RoleId};
use safety_broker_contract::TaintSource;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::grant::RunGrant;

/// Nazwa wbudowanego narzędzia delegacji (grupa ról `delegate`: Dyrygentka, Mówczyni).
pub const DELEGATE_TOOL: &str = "delegate_task";

/// Grupa narzędzi ról, które mogą delegować.
pub const DELEGATE_GROUP: &str = "delegate";

/// Obsada widziana przez przebieg: kto gra jakie role (delegacja do roli, wybór Krytyczki
/// „Krytyczka ≠ autorka” przez [`Cast::verifier_for`]) i modele per rola.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Crew {
    /// Obsada sesji.
    pub cast: Cast,
    /// Persony (wbudowane i z Kreatora) — dane do promptu podprzebiegu.
    pub personas: Vec<Persona>,
    /// Role (wbudowane i własne).
    pub roles: Vec<Role>,
    /// Profile narzędzi person, przypięte przez właściciela przy starcie przebiegu.
    /// Pusta mapa zachowuje zachowanie starszych checkpointów.
    #[serde(default)]
    pub persona_roles: BTreeMap<PersonaId, Vec<Role>>,
    /// Instrukcje współpracy person dla nowych delegacji; bez dziedziczenia stylu rodzica.
    /// Pusta mapa zachowuje zachowanie starszych checkpointów i format Krytyczki.
    #[serde(default)]
    pub persona_instructions: BTreeMap<PersonaId, String>,
    /// Model per rola (brak = model przebiegu-rodzica).
    #[serde(default)]
    pub models: BTreeMap<RoleId, String>,
}

impl Crew {
    /// Rola z uwzględnieniem profilu dostępu konkretnej persony.
    pub fn role_for(&self, persona: &PersonaId, id: &RoleId) -> Option<&Role> {
        match self.persona_roles.get(persona) {
            Some(roles) => roles.iter().find(|r| &r.id == id),
            None => self.role(id),
        }
    }
    /// Persona po identyfikatorze.
    pub fn persona(&self, id: &PersonaId) -> Option<&Persona> {
        self.personas.iter().find(|p| &p.id == id)
    }

    /// Rola po identyfikatorze.
    pub fn role(&self, id: &RoleId) -> Option<&Role> {
        self.roles.iter().find(|r| &r.id == id)
    }
}

/// Opcje przebiegu v1 (wszystkie domyślne = zachowanie v0).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct RunOptions {
    /// Jawne zgody właściciela dotyczące miękkich limitów.
    #[serde(default)]
    pub continuation: crate::ContinuationPolicy,
    /// Prywatność żądań, także odtwarzanych z lokalnego punktu.
    #[serde(default)]
    pub request_privacy: providers_contract::PrivacyTag,
    /// Tura rozmowy, z której uruchomiono zadanie (punkt wznowienia po limicie).
    #[serde(default)]
    pub owner_turn: Option<String>,
    /// Zlecenie nadrzędne, pozwala powiązać checkpoint z konkretnym etapem.
    #[serde(default)]
    pub owner_call: Option<String>,
    /// Przebieg-rodzic (delegacja, Krytyczka, umiejętność uruchomiona przez agentkę).
    #[serde(default)]
    pub parent: Option<RunId>,
    /// Głębokość delegacji (0 = przebieg główny).
    #[serde(default)]
    pub depth: u32,
    /// Koperta uprawnień (`None` = narzędzia z `RunSpec` przefiltrowane rolami, jak w v0).
    #[serde(default)]
    pub grant: Option<RunGrant>,
    /// Obsada: włącza delegację (rola z grupą `delegate`) i Krytyczkę zamiast samoweryfikacji.
    #[serde(default)]
    pub crew: Option<Crew>,
    /// Taint odziedziczony po rodzicu (sesja widziała niezaufaną treść).
    #[serde(default)]
    pub inherited_taint: Option<TaintSource>,
    /// Treść zaufana rodzica (cel i wiadomości właściciela) — proweniencja argumentów potomka.
    #[serde(default)]
    pub trusted_context: String,
    /// Treść niezaufana widziana przez rodzica — proweniencja argumentów potomka.
    #[serde(default)]
    pub untrusted_context: String,
    /// Etykieta dla UI i raportu (np. „umiejętność: porządki w Pobranych”).
    #[serde(default)]
    pub label: Option<String>,
}

/// Argumenty narzędzia [`DELEGATE_TOOL`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DelegateArgs {
    /// Rola, której przekazujesz zadanie (np. `operator`, `writer`, `researcher`).
    pub role: String,
    /// Konkretna agentka (`delta`…); domyślnie pierwsza z obsady grająca tę rolę.
    #[serde(default)]
    pub persona: Option<String>,
    /// Zadanie dla niej — samodzielne, konkretne, z kryterium „gotowe”.
    pub goal: String,
    /// Zawężenie narzędzi (podzbiór Twoich); domyślnie wszystkie dozwolone dla roli.
    #[serde(default)]
    pub tools: Option<Vec<String>>,
    /// Limit kroków podzadania (≤ Twój pozostały budżet).
    #[serde(default)]
    pub max_steps: Option<u32>,
}

/// Ładunek zadania schedulera wykonywanego pętlą agentki (`TaskSpec::payload`, wykonawca
/// `ExecutorKind::Agent`). Adapter `TaskExecutor` w `agent-runtime-impl` podmienia agentkę na
/// przydzieloną przez scheduler (z obsady), zawęża budżety do budżetu zadania i dziedziczy taint.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct AgentTaskPayload {
    /// Specyfikacja przebiegu.
    pub spec: crate::spec::RunSpec,
    /// Opcje v1.
    #[serde(default)]
    pub options: RunOptions,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crew_instructions_round_trip_and_old_checkpoint_defaults_to_empty() {
        let mut crew = Crew::default();
        crew.persona_instructions
            .insert(PersonaId::delta(), "Krótki meldunek.".into());
        let mut value = serde_json::to_value(&crew).unwrap();
        assert_eq!(serde_json::from_value::<Crew>(value.clone()).unwrap(), crew);
        value
            .as_object_mut()
            .unwrap()
            .remove("persona_instructions");
        assert!(
            serde_json::from_value::<Crew>(value)
                .unwrap()
                .persona_instructions
                .is_empty()
        );
    }
}
