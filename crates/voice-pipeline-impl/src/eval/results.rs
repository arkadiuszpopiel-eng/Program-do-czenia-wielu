//! Wyniki pozycji (NDJSON) — produkuje runner offline / Voice Lab na prawdziwych modelach,
//! czyta `alfa-voice-eval score`.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use voice_cmd_contract::CommandKind;
use voice_dialog_contract::InterruptIntent;

/// Wynik jednej pozycji manifestu.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ItemResult {
    /// Identyfikator pozycji manifestu.
    pub id: String,
    /// Transkrypt końcowy STT.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hypothesis: Option<String>,
    /// Wykryta komenda szybka.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub command: Option<CommandKind>,
    /// Reakcja: wykrycie komendy − początek słowa (ms).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reaction_ms: Option<u64>,
    /// Czy wypowiedź w trakcie mowy agentki ją przerwała (twardy stop).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub interrupted: Option<bool>,
    /// Sklasyfikowana intencja przerwania.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub intent: Option<InterruptIntent>,
    /// Usłyszany prefiks wg potoku (słowa).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub heard_words: Option<usize>,
}

/// Wczytuje wyniki NDJSON.
pub fn parse_results(text: &str) -> Result<Vec<ItemResult>, String> {
    text.lines()
        .enumerate()
        .filter(|(_, l)| !l.trim().is_empty())
        .map(|(i, l)| serde_json::from_str(l).map_err(|e| format!("wyniki, linia {}: {e}", i + 1)))
        .collect()
}

/// Zapisuje wyniki jako NDJSON.
pub fn to_ndjson(results: &[ItemResult]) -> String {
    results
        .iter()
        .filter_map(|r| serde_json::to_string(r).ok())
        .map(|l| l + "\n")
        .collect()
}

/// JSON Schema linii wyników (`evals/F2/results.schema.json`).
pub fn results_schema() -> serde_json::Value {
    serde_json::to_value(schemars::schema_for!(ItemResult)).unwrap_or_default()
}
