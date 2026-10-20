//! Schematy JSON wymuszania wywołań narzędzi na `llama-server` (C2.3): parametry narzędzia
//! gotowe do gramatyki (odwołania `$ref` rozwinięte, obiekty z `properties` domknięte
//! `additionalProperties: false`), koperta odpowiedzi do `response_format` i walidacja argumentów.
//!
//! `llama-server` zamienia schemat z `response_format` na gramatykę GBNF i próbkuje tylko tokeny,
//! które ją spełniają — dlatego schemat musi być samowystarczalny (bez `$defs` poza korzeniem
//! koperty) i jawnie domknięty, niezależnie od tego, jak dana wersja serwera traktuje brak
//! `additionalProperties`. Walidacja argumentów w Alfie używa schematu narzędzia bez domykania
//! ([`validation_parameters`]): dodatkowe pole odrzuca tylko narzędzie, które samo tego chce.
//!
//! Schematy pochodzą też od niezaufanych serwerów MCP, więc rozwijanie `$ref` ma twarde granice:
//! odwołanie rekurencyjne (definicja już rozwijana na bieżącej ścieżce) i każde odwołanie po
//! wyczerpaniu budżetu węzłów albo głębokości → `{}` (dowolna wartość).

use std::collections::BTreeMap;

use providers_contract::{ToolChoice, ToolSpec};
use serde_json::{Map, Value, json};

/// Najgłębsze zagnieżdżenie rozwijanych `$ref` na jednej ścieżce.
const MAX_REF_DEPTH: usize = 8;
/// Budżet węzłów schematu jednego narzędzia: po jego wyczerpaniu kolejne `$ref` → `{}`.
const MAX_NODES: usize = 4096;
/// Najdłuższy opis błędów walidacji odsyłany modelowi (znaki).
const MAX_ERROR_CHARS: usize = 400;
/// Najwięcej błędów walidacji w jednym opisie.
const MAX_ERRORS: usize = 3;
/// Słowa kluczowe dokumentu, zbędne w schemacie rozwiniętym.
const DOCUMENT_KEYS: [&str; 4] = ["$schema", "$id", "$defs", "definitions"];
/// Słowa kluczowe z wartościami (nie schematami) — bez rozwijania i domykania.
const VALUE_KEYS: [&str; 5] = ["enum", "const", "default", "examples", "required"];

/// Schemat parametrów narzędzia gotowy do gramatyki: rozwinięty i domknięty. Brak schematu
/// obiektu (albo schemat innego typu) → dowolny obiekt.
pub(crate) fn tool_parameters(spec: &ToolSpec) -> Value {
    let mut schema = inlined_parameters(spec);
    close_objects(&mut schema, true);
    schema
}

/// Schemat parametrów do walidacji argumentów: rozwinięty, ale NIE domknięty — dodatkowe pole
/// odrzuca tylko schemat, który sam tego chce (`additionalProperties: false` narzędzia).
/// Gramatyka i tak nie pozwala modelowi dopisać pól; gdy serwer gramatyki nie zastosuje,
/// nieszkodliwe dodatkowe pole nie może zablokować wywołania (regresja próby obowiązków 0.0.64).
pub(crate) fn validation_parameters(spec: &ToolSpec) -> Value {
    inlined_parameters(spec)
}

/// Parametry do walidacji wg nazwy narzędzia ([`validation_parameters`]).
pub(crate) fn validation_of(tools: &[&ToolSpec]) -> BTreeMap<String, Value> {
    tools
        .iter()
        .map(|t| (t.name.clone(), validation_parameters(t)))
        .collect()
}

/// Schemat parametrów z rozwiniętymi odwołaniami, bez słów kluczowych dokumentu.
fn inlined_parameters(spec: &ToolSpec) -> Value {
    let root = &spec.input_schema;
    if !root.is_object() {
        return json!({"type": "object"});
    }
    let mut inliner = Inliner {
        root,
        // Korzeń jest rozwijany od początku: odwołanie `#` to już rekurencja.
        path: vec![""],
        nodes: 0,
    };
    let mut schema = inliner.value(root);
    if let Some(obj) = schema.as_object_mut() {
        for key in DOCUMENT_KEYS.iter().chain(&["title"]) {
            obj.remove(*key);
        }
    }
    schema
}

/// Parametry oferowanych narzędzi wg nazwy (liczone raz na żądanie).
pub(crate) fn parameters_of(tools: &[&ToolSpec]) -> BTreeMap<String, Value> {
    tools
        .iter()
        .map(|t| (t.name.clone(), tool_parameters(t)))
        .collect()
}

/// Rozwijanie lokalnych odwołań (`#`, `#/$defs/…`, `#/definitions/…`) z granicami.
struct Inliner<'a> {
    root: &'a Value,
    /// Wskaźniki definicji rozwijanych na bieżącej ścieżce.
    path: Vec<&'a str>,
    /// Węzły już skopiowane.
    nodes: usize,
}

impl<'a> Inliner<'a> {
    fn value(&mut self, value: &'a Value) -> Value {
        self.nodes = self.nodes.saturating_add(1);
        match value {
            Value::Object(obj) => match obj.get("$ref") {
                Some(Value::String(pointer)) => self.reference(pointer, obj),
                _ => Value::Object(self.members(obj)),
            },
            Value::Array(items) => Value::Array(items.iter().map(|v| self.value(v)).collect()),
            other => other.clone(),
        }
    }

    /// Pola obiektu schematu (bez `$ref`); wartości słów kluczowych z danymi — bez zmian.
    fn members(&mut self, obj: &'a Map<String, Value>) -> Map<String, Value> {
        obj.iter()
            .filter(|(k, _)| k.as_str() != "$ref")
            .map(|(k, v)| {
                let v = if VALUE_KEYS.contains(&k.as_str()) {
                    v.clone()
                } else {
                    self.value(v)
                };
                (k.clone(), v)
            })
            .collect()
    }

    /// Odwołanie: cel rozwinięty (rekurencja, brak celu, wyczerpany budżet → `{}`) i scalony
    /// z pozostałymi słowami kluczowymi obiektu z `$ref`.
    fn reference(&mut self, pointer: &'a str, obj: &'a Map<String, Value>) -> Value {
        let target = pointer
            .strip_prefix('#')
            .and_then(|k| self.root.pointer(k).map(|t| (k, t)));
        let resolved = match target {
            Some((key, target))
                if !self.path.contains(&key)
                    && self.path.len() <= MAX_REF_DEPTH
                    && self.nodes < MAX_NODES =>
            {
                self.path.push(key);
                let mut resolved = self.value(target);
                self.path.pop();
                if let Some(obj) = resolved.as_object_mut() {
                    for key in DOCUMENT_KEYS {
                        obj.remove(key);
                    }
                }
                resolved
            }
            _ => json!({}),
        };
        let siblings = self.members(obj);
        merge(resolved, siblings)
    }
}

/// Scala cel `$ref` ze słowami kluczowymi obok odwołania: pola i wymagane — sumą, reszta
/// (opisy, ograniczenia) — wartością obok odwołania.
fn merge(resolved: Value, siblings: Map<String, Value>) -> Value {
    if siblings.is_empty() {
        return resolved;
    }
    let mut base = match resolved {
        Value::Object(obj) => obj,
        Value::Bool(false) => return Value::Bool(false),
        _ => Map::new(),
    };
    for (key, value) in siblings {
        let rest = match (key.as_str(), base.get_mut(&key), value) {
            ("properties", Some(Value::Object(into)), Value::Object(add)) => {
                into.extend(add);
                None
            }
            ("required", Some(Value::Array(into)), Value::Array(add)) => {
                for name in add {
                    if !into.contains(&name) {
                        into.push(name);
                    }
                }
                None
            }
            (_, _, value) => Some(value),
        };
        if let Some(value) = rest {
            base.insert(key, value);
        }
    }
    Value::Object(base)
}

/// Grupy słów kluczowych stosujących podschematy do tej samej instancji: składniki grupy są
/// alternatywami (`anyOf`, `oneOf`, `then`/`else`) albo obowiązują razem (`allOf`,
/// `dependentSchemas`); grupy obowiązują razem.
const IN_PLACE: [&[&str]; 5] = [
    &["allOf"],
    &["anyOf"],
    &["oneOf"],
    &["then", "else"],
    &["dependentSchemas"],
];

/// Podschematy wartości słowa kluczowego (tablica, mapa `dependentSchemas` albo schemat).
fn subschemas<'a>(key: &str, value: &'a Value) -> Vec<&'a Value> {
    match (key, value) {
        (_, Value::Array(items)) => items.iter().collect(),
        ("dependentSchemas", Value::Object(map)) => map.values().collect(),
        (_, other) => vec![other],
    }
}

/// Czy podschematy grupy dokładają pola tej samej instancji.
fn group_adds(obj: &Map<String, Value>, group: &[&str]) -> bool {
    group.iter().any(|k| {
        obj.get(*k)
            .is_some_and(|v| subschemas(k, v).into_iter().any(adds_properties))
    })
}

/// Czy schemat dopuszcza pola (własne albo przez podschematy tej samej instancji).
fn adds_properties(value: &Value) -> bool {
    let Value::Object(obj) = value else {
        return false;
    };
    obj.contains_key("properties")
        || obj.contains_key("patternProperties")
        || obj
            .get("additionalProperties")
            .is_some_and(|v| v != &Value::Bool(false))
        || IN_PLACE.iter().any(|group| group_adds(obj, group))
}

/// Domyka obiekty z `properties` bez `additionalProperties`/`patternProperties`, gdy żaden
/// podschemat tej samej instancji (`allOf`, `anyOf`, `oneOf`, `then`/`else`, `dependentSchemas`)
/// nie dokłada pól — inaczej obiekt zostaje otwarty (domknięte części wykluczałyby się nawzajem).
/// Składniki `allOf` i `dependentSchemas` zostają otwarte; alternatywy (`anyOf`, `oneOf`,
/// `then`/`else`) są domykane tylko pod obiektem bez własnych pól, gdy pola dokłada jedna grupa.
/// Warunki (`if`) i zaprzeczenia (`not`) zostają bez zmian — domknięcie odwróciłoby ich sens.
fn close_objects(value: &mut Value, close_here: bool) {
    let obj = match value {
        Value::Object(obj) => obj,
        Value::Array(items) => {
            items.iter_mut().for_each(|v| close_objects(v, close_here));
            return;
        }
        _ => return,
    };
    let adding = IN_PLACE
        .iter()
        .filter(|group| group_adds(obj, group))
        .count();
    let own = obj.contains_key("properties") || obj.contains_key("patternProperties");
    if close_here
        && obj.contains_key("properties")
        && !obj.contains_key("additionalProperties")
        && !obj.contains_key("patternProperties")
        && adding == 0
    {
        obj.insert("additionalProperties".into(), Value::Bool(false));
    }
    let close_branches = close_here && !own && adding <= 1;
    for (key, child) in obj.iter_mut() {
        match key.as_str() {
            // Mapy „nazwa → schemat": domykamy schematy, nie samą mapę.
            "properties" | "patternProperties" | "$defs" | "definitions" => {
                if let Value::Object(map) = child {
                    map.values_mut().for_each(|v| close_objects(v, true));
                }
            }
            "dependentSchemas" => {
                if let Value::Object(map) = child {
                    map.values_mut().for_each(|v| close_objects(v, false));
                }
            }
            "allOf" => close_objects(child, false),
            "anyOf" | "oneOf" | "then" | "else" => close_objects(child, close_branches),
            "if" | "not" => {}
            k if VALUE_KEYS.contains(&k) => {}
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
pub(crate) fn envelope(parameters: &BTreeMap<String, Value>, must_call: bool) -> Value {
    let mut calls: Vec<Value> = parameters
        .iter()
        .map(|(name, params)| {
            let mut properties = Map::new();
            properties.insert("name".into(), json!({"const": name}));
            properties.insert("parameters".into(), params.clone());
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
