//! Pipeline modeli: nazwany łańcuch etapów, w którym każdy etap wykonuje wskazana rola (agentka)
//! na wskazanym modelu, a wynik etapu trafia do następnego (`run_pipeline` Dyrygentki).

use serde::{Deserialize, Serialize};

/// Etap pipeline'u.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct PipelineStage {
    /// Krótka nazwa etapu pokazywana w turze i w Replay (pusta = z roli).
    pub title: String,
    /// Rola wykonawcza (`coder`, `operator`, `researcher`, `critic`, `thinker`, `writer`, `keeper`).
    pub role: String,
    /// Opcjonalna agentka (`delta`, `gama`, `beta`); pusta — według roli.
    pub persona: Option<String>,
    /// Opcjonalny model `dostawca:model`; pusty — dobór Routera.
    pub model: Option<String>,
    /// Zadanie etapu; może zawierać `{{input}}` (wejście) i `{{prev}}` (wynik poprzedniego etapu).
    pub goal: String,
}

/// Zapisany lub wbudowany pipeline.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct PipelineDef {
    /// Unikalna nazwa (klucz).
    pub name: String,
    /// Krótki opis do czego służy.
    pub description: String,
    /// Etapy wykonywane po kolei (1–6).
    pub stages: Vec<PipelineStage>,
    /// Wbudowany wzorzec (nieedytowalny; zapis pod tą samą nazwą tworzy własną wersję).
    pub builtin: bool,
}
