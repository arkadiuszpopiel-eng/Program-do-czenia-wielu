//! Wspólna obróbka wywołania (impl i fake): kontrola wejścia, deserializacja wyniku wtyczki
//! (treść złośliwa = błąd, nigdy panika), wynik narzędzia oznaczony jako niezaufany,
//! statystyki do zdarzeń `plugin.invoked` / `plugin.trapped`.

use core_bus_contract::{Event, Level};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tools_common_contract::ToolOutcome;
use tools_common_contract::text::{redact_secrets, truncate_chars};

use crate::error::ExecError;
use crate::manifest::{PluginLimits, PluginManifest, PluginToolDecl, UNTRUSTED_SOURCE};
use crate::{event_kind, events};

/// Najdłuższy tekst wyniku dla modelu (znaki).
pub const MAX_TEXT_CHARS: usize = 16_000;
/// Najwięcej wpisów `log` w raporcie wywołania.
pub const MAX_LOGS: usize = 16;

/// Statystyki wywołania (ładunek zdarzeń; bez treści wejścia/wyjścia).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct InvocationStats {
    /// Wtyczka.
    pub plugin: String,
    /// Wersja.
    pub version: String,
    /// Narzędzie (nazwa w wtyczce).
    pub tool: String,
    /// Zużyte paliwo.
    pub fuel_used: u64,
    /// Czas wykonania Wasm (ms, bez operacji hosta).
    pub wasm_ms: u64,
    /// Operacje hosta.
    pub host_calls: u32,
    /// Operacje hosta odrzucone (manifest, Broker).
    pub host_denied: u32,
    /// Wpisy `log` (obcięte, zredagowane).
    pub logs: Vec<String>,
    /// Wynik: `ok` albo rodzaj błędu ([`ExecError::kind`]).
    pub result: String,
}

impl InvocationStats {
    /// Statystyki początkowe dla narzędzia.
    pub fn new(manifest: &PluginManifest, tool: &str) -> Self {
        Self {
            plugin: manifest.id.to_string(),
            version: manifest.version.to_string(),
            tool: tool.to_owned(),
            ..Self::default()
        }
    }

    /// Zdarzenie `plugin.invoked` albo — dla przerwania przez piaskownicę — `plugin.trapped`.
    pub fn event(&self, error: Option<&ExecError>) -> Event {
        let (name, level) = match error {
            Some(e) if e.is_sandbox_stop() => (events::TRAPPED, Level::Warn),
            _ => (events::INVOKED, Level::Debug),
        };
        let payload = serde_json::to_value(self).unwrap_or_default();
        Event::new(event_kind(name), level, payload)
    }
}

/// Kontrola rozmiaru wejścia przed uruchomieniem.
pub fn check_input(input: &str, limits: &PluginLimits) -> Result<(), ExecError> {
    let limit = usize::try_from(limits.max_input_bytes).unwrap_or(usize::MAX);
    if input.len() > limit {
        return Err(ExecError::InputTooLarge {
            bytes: input.len(),
            limit,
        });
    }
    Ok(())
}

fn type_matches(schema: &Value, value: &Value) -> bool {
    match schema.get("type").and_then(Value::as_str) {
        Some("object") => value.is_object(),
        Some("array") => value.is_array(),
        Some("string") => value.is_string(),
        Some("number") => value.is_number(),
        Some("integer") => value.is_i64() || value.is_u64(),
        Some("boolean") => value.is_boolean(),
        _ => true,
    }
}

/// Wynik wtyczki → dane narzędzia: limit rozmiaru, ścisły JSON (limit głębokości serde),
/// zgodność typu z `output_schema` i wymagane pola. Treść zawsze niezaufana.
pub fn parse_output(
    output: &str,
    decl: &PluginToolDecl,
    limits: &PluginLimits,
) -> Result<Value, ExecError> {
    let limit = usize::try_from(limits.max_output_bytes).unwrap_or(usize::MAX);
    if output.len() > limit {
        return Err(ExecError::OutputTooLarge {
            bytes: output.len(),
            limit,
        });
    }
    let value: Value =
        serde_json::from_str(output).map_err(|e| ExecError::InvalidOutput(e.to_string()))?;
    if !type_matches(&decl.output_schema, &value) {
        return Err(ExecError::InvalidOutput(
            "typ wyniku niezgodny z `output_schema`".into(),
        ));
    }
    let required = decl
        .output_schema
        .get("required")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str);
    for field in required {
        if value.get(field).is_none() {
            return Err(ExecError::InvalidOutput(format!("brak pola `{field}`")));
        }
    }
    Ok(value)
}

/// Wynik narzędzia z danych wtyczki (tekst zredagowany i obcięty, oznaczony jako niezaufany).
pub fn ok_outcome(data: Value) -> ToolOutcome {
    let raw = data.to_string();
    let (text, truncated) = truncate_chars(&redact_secrets(&raw), MAX_TEXT_CHARS);
    let mut out = ToolOutcome::ok(text, data).untrusted(UNTRUSTED_SOURCE);
    out.truncated = truncated;
    out
}
