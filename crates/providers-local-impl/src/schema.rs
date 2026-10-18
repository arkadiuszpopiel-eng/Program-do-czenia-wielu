//! Schematy JSON wymuszania wywołań narzędzi na `llama-server` (C2.3): parametry narzędzia
//! gotowe do gramatyki (odwołania `$ref` rozwinięte, obiekty z `properties` domknięte
//! `additionalProperties: false`), koperta odpowiedzi do `response_format` i walidacja argumentów.
//!
//! `llama-server` zamienia schemat z `response_format` na gramatykę GBNF i próbkuje tylko tokeny,
//! które ją spełniają — dlatego schemat musi być samowystarczalny (bez `$defs` poza korzeniem
//! koperty) i jawnie domknięty: gramatyka serwera i walidacja w Alfie oceniają wtedy to samo,
//! niezależnie od tego, jak dana wersja serwera traktuje brak `additionalProperties`.

use providers_contract::{ToolChoice, ToolSpec};
use serde_json::{Map, Value, json};

/// Najgłębsze rozwijanie `$ref` — schematy rekurencyjne dalej przyjmują dowolną wartość.
const MAX_REF_DEPTH: usize = 8;
/// Najdłuższy opis błędów walidacji odsyłany modelowi (znaki).
const MAX_ERROR_CHARS: usize = 400;
/// Najwięcej błędów walidacji w jednym opisie.
const MAX_ERRORS: usize = 3;

/// Schemat parametrów narzędzia gotowy do gramatyki i walidacji. Brak schematu obiektu (albo
/// schemat innego typu) → dowolny obiekt.
pub(crate) fn tool_parameters(spec: &ToolSpec) -> Value {
    let root = &spec.input_schema;
    if !root.is_object() {
        return json!({"type": "object"});
    }
    let mut schema = inline(root, root, 0);
    if let Some(obj) = schema.as_object_mut() {
        for key in ["$schema", "$id", "$defs", "definitions", "title"] {
            obj.remove(key);
        }
    }
    close_objects(&mut schema, true);
    schema
}

/// Rozwija lokalne odwołania (`#/$defs/…`, `#/definitions/…`); nierozwiązywalne → `{}`.
fn inline(value: &Value, root: &Value, depth: usize) -> Value {
    match value {
        Value::Object(obj) => {
            if let Some(Value::String(pointer)) = obj.get("$ref") {
                let target = pointer.strip_prefix('#').and_then(|p| root.pointer(p));
                let Some(target) = target.filter(|_| depth < MAX_REF_DEPTH) else {
                    return json!({});
                };
                let mut resolved = inline(target, root, depth + 1);
                if let Some(obj) = resolved.as_object_mut() {
                    // Kopia korzenia (odwołanie `#`) nie przenosi metadanych dokumentu.
                    for key in ["$schema", "$id", "$defs", "definitions"] {
                        obj.remove(key);
                    }
                }
                return resolved;
            }
            Value::Object(
                obj.iter()
                    .map(|(k, v)| (k.clone(), inline(v, root, depth)))
                    .collect(),
            )
        }
        Value::Array(items) => Value::Array(items.iter().map(|v| inline(v, root, depth)).collect()),
        other => other.clone(),
    }
}

/// Domyka obiekty z `properties` bez `additionalProperties`/`patternProperties`. Składniki
/// `allOf` zostają otwarte (domknięte wykluczałyby się nawzajem), ich pola — już nie.
fn close_objects(value: &mut Value, close_here: bool) {
    let obj = match value {
        Value::Object(obj) => obj,
        Value::Array(items) => {
            items.iter_mut().for_each(|v| close_objects(v, close_here));
            return;
        }
        _ => return,
    };
    if close_here
        && obj.contains_key("properties")
        && !obj.contains_key("additionalProperties")
        && !obj.contains_key("patternProperties")
    {
        obj.insert("additionalProperties".into(), Value::Bool(false));
    }
    for (key, child) in obj.iter_mut() {
        match key.as_str() {
            // Mapy „nazwa → schemat": domykamy schematy, nie samą mapę.
            "properties" | "patternProperties" | "dependentSchemas" | "$defs" | "definitions" => {
                if let Value::Object(map) = child {
                    map.values_mut().for_each(|v| close_objects(v, true));
                }
            }
            "allOf" => close_objects(child, false),
            // Wartości, nie schematy.
            "enum" | "const" | "default" | "examples" | "required" => {}
            _ => close_objects(child, true),
        }
    }
}

/// Narzędzia, które model może wywołać: przy `ToolChoice::Tool` tylko wskazane (gdy istnieje),
/// inaczej wszystkie — w stałej kolejności nazw (powtarzalny prompt i gramatyka).
pub(crate) fn offered<'a>(tools: &'a [ToolSpec], choice: &ToolChoice) -> Vec<&'a ToolSpec> {
    let mut list: Vec<&ToolSpec> = match choice {
        ToolChoice::Tool { name } if tools.iter().any(|t| &t.name == name) => {
            tools.iter().filter(|t| &t.name == name).collect()
        }
        _ => tools.iter().collect(),
    };
    list.sort_by(|a, b| a.name.cmp(&b.name));
    list
}

/// Czy model musi wywołać narzędzie (bez gałęzi `response`).
pub(crate) fn must_call(choice: &ToolChoice) -> bool {
    matches!(choice, ToolChoice::Required | ToolChoice::Tool { .. })
}

fn closed_object(properties: Value, required: &[&str]) -> Value {
    json!({
        "type": "object",
        "properties": properties,
        "required": required,
        "additionalProperties": false,
    })
}

/// Schemat koperty: `{"response": "…", "tool_call": {"name": …, "parameters": {…}}}` — tekst dla
/// użytkownika (odpowiedź albo plan przed wywołaniem; może być pusty) i opcjonalnie jedno
/// wywołanie (`must_call` — obowiązkowe). Gramatyka zachowuje kolejność pól, więc model pisze
/// najpierw tekst, potem wywołanie; nazwa jest stałą (`const`), więc argumenty idą zawsze wg
/// schematu wybranego narzędzia.
pub(crate) fn envelope(tools: &[&ToolSpec], must_call: bool) -> Value {
    let mut calls: Vec<Value> = tools
        .iter()
        .map(|t| {
            let mut properties = Map::new();
            properties.insert("name".into(), json!({"const": t.name}));
            properties.insert("parameters".into(), tool_parameters(t));
            closed_object(Value::Object(properties), &["name", "parameters"])
        })
        .collect();
    let call = if calls.len() == 1 {
        calls.pop().unwrap_or_default()
    } else {
        json!({"anyOf": calls})
    };
    let mut properties = Map::new();
    properties.insert("response".into(), json!({"type": "string"}));
    properties.insert("tool_call".into(), call);
    let required: &[&str] = if must_call {
        &["response", "tool_call"]
    } else {
        &["response"]
    };
    closed_object(Value::Object(properties), required)
}

/// Pole `response_format` żądania (`llama-server`: `json_schema.schema` → gramatyka).
pub(crate) fn response_format(schema: Value) -> Value {
    json!({
        "type": "json_schema",
        "json_schema": {"name": "alfa_koperta", "strict": true, "schema": schema},
    })
}

/// Sprawdza argumenty względem schematu narzędzia; `Err` — zwięzły opis dla modelu. Schemat,
/// którego walidator nie rozumie, nie blokuje wywołania (rozstrzyga samo narzędzie).
pub(crate) fn check_arguments(schema: &Value, args: &Value) -> Result<(), String> {
    let Ok(validator) = jsonschema::validator_for(schema) else {
        return Ok(());
    };
    let errors: Vec<String> = validator
        .iter_errors(args)
        .take(MAX_ERRORS)
        .map(|e| {
            let path = e.instance_path().to_string();
            if path.is_empty() {
                e.to_string()
            } else {
                format!("{path}: {e}")
            }
        })
        .collect();
    if errors.is_empty() {
        return Ok(());
    }
    let text = errors.join("; ");
    Err(match text.char_indices().nth(MAX_ERROR_CHARS) {
        Some((at, _)) => format!("{}…", text.get(..at).unwrap_or_default()),
        None => text,
    })
}

#[cfg(test)]
#[path = "schema_tests.rs"]
mod tests;
