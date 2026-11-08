//! Parametry umiejętności: podzbiór JSON Schema (obiekt zamknięty; `string|integer|number|
//! boolean|array` napisów; `enum`, `required`, `default`, `maxLength`, `minimum`, `maximum`,
//! `maxItems`) — walidacja schematu, walidacja i normalizacja wartości, szablon `{{parametr}}`.

use serde_json::{Map, Value};

/// Domyślny limit długości napisu parametru.
pub const DEFAULT_MAX_LEN: u64 = 2000;
/// Najwięcej parametrów.
pub const MAX_PARAMS: usize = 32;

fn valid_name(n: &str) -> bool {
    n.chars()
        .next()
        .is_some_and(|c| c.is_ascii_lowercase() || c == '_')
        && n.len() <= 32
        && n.chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
}

fn props(schema: &Value) -> Result<&Map<String, Value>, String> {
    if schema.get("type") != Some(&Value::from("object")) {
        return Err("schemat parametrów musi być obiektem (`type: object`)".into());
    }
    if schema.get("additionalProperties") != Some(&Value::Bool(false)) {
        return Err("schemat parametrów musi mieć `additionalProperties: false`".into());
    }
    match schema.get("properties") {
        None => Ok(EMPTY.get_or_init(Map::new)),
        Some(Value::Object(m)) => Ok(m),
        Some(_) => Err("`properties` musi być obiektem".into()),
    }
}

static EMPTY: std::sync::OnceLock<Map<String, Value>> = std::sync::OnceLock::new();

fn required(schema: &Value) -> Vec<String> {
    schema
        .get("required")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str().map(str::to_owned))
                .collect()
        })
        .unwrap_or_default()
}

/// Sprawdza schemat parametrów (obsługiwany podzbiór).
pub fn validate_schema(schema: &Value) -> Result<(), String> {
    let p = props(schema)?;
    if p.len() > MAX_PARAMS {
        return Err(format!("za dużo parametrów (> {MAX_PARAMS})"));
    }
    for (name, prop) in p {
        if !valid_name(name) {
            return Err(format!("niepoprawna nazwa parametru `{name}`"));
        }
        let ty = prop.get("type").and_then(Value::as_str).unwrap_or("");
        match ty {
            "string" | "integer" | "number" | "boolean" => {}
            "array" => {
                let item = prop
                    .get("items")
                    .and_then(|i| i.get("type"))
                    .and_then(Value::as_str);
                if item != Some("string") {
                    return Err(format!("`{name}`: tablice tylko napisów"));
                }
            }
            other => return Err(format!("`{name}`: nieobsługiwany typ `{other}`")),
        }
        if let Some(d) = prop.get("default") {
            check(name, prop, d).map_err(|e| format!("wartość domyślna: {e}"))?;
        }
    }
    for r in required(schema) {
        if !p.contains_key(&r) {
            return Err(format!("`required` wskazuje nieznany parametr `{r}`"));
        }
    }
    Ok(())
}

fn check(name: &str, prop: &Value, v: &Value) -> Result<(), String> {
    let ty = prop.get("type").and_then(Value::as_str).unwrap_or("");
    let ok = match ty {
        "string" => v.as_str().is_some_and(|s| {
            let max = prop
                .get("maxLength")
                .and_then(Value::as_u64)
                .unwrap_or(DEFAULT_MAX_LEN);
            (s.chars().count() as u64) <= max
        }),
        "integer" => v.as_i64().is_some() || v.as_u64().is_some(),
        "number" => v.is_number(),
        "boolean" => v.is_boolean(),
        "array" => v.as_array().is_some_and(|a| {
            let max = prop.get("maxItems").and_then(Value::as_u64).unwrap_or(64);
            a.len() as u64 <= max && a.iter().all(Value::is_string)
        }),
        _ => false,
    };
    if !ok {
        return Err(format!(
            "parametr `{name}` ma zły typ albo za długą wartość"
        ));
    }
    if let Some(n) = v.as_f64() {
        let lo = prop.get("minimum").and_then(Value::as_f64);
        let hi = prop.get("maximum").and_then(Value::as_f64);
        if lo.is_some_and(|lo| n < lo) || hi.is_some_and(|hi| n > hi) {
            return Err(format!("parametr `{name}` poza zakresem"));
        }
    }
    if let Some(allowed) = prop.get("enum").and_then(Value::as_array)
        && !allowed.contains(v)
    {
        return Err(format!("parametr `{name}` spoza dozwolonych wartości"));
    }
    Ok(())
}

/// Waliduje i normalizuje parametry (`null` = brak; wartości domyślne dopisane).
pub fn validate_params(schema: &Value, params: &Value) -> Result<Map<String, Value>, String> {
    let p = props(schema)?;
    let given = match params {
        Value::Null => Map::new(),
        Value::Object(m) => m.clone(),
        _ => return Err("parametry muszą być obiektem".into()),
    };
    if let Some(extra) = given.keys().find(|k| !p.contains_key(*k)) {
        return Err(format!("nieznany parametr `{extra}`"));
    }
    let mut out = Map::new();
    for (name, prop) in p {
        match given.get(name).or_else(|| prop.get("default")) {
            Some(v) => {
                check(name, prop, v)?;
                out.insert(name.clone(), v.clone());
            }
            None if required(schema).contains(name) => {
                return Err(format!("brak wymaganego parametru `{name}`"));
            }
            None => {}
        }
    }
    Ok(out)
}

/// Znaczniki `{{nazwa}}` w szablonie (w kolejności, bez powtórzeń).
pub fn placeholders(template: &str) -> Result<Vec<String>, String> {
    let mut out: Vec<String> = Vec::new();
    let mut rest = template;
    while let Some(open) = rest.find("{{") {
        let after = &rest[open + 2..];
        let close = after.find("}}").ok_or("niedomknięty znacznik `{{`")?;
        let name = after[..close].trim();
        if !valid_name(name) {
            return Err(format!("niepoprawny znacznik `{{{{{name}}}}}`"));
        }
        if !out.iter().any(|n| n == name) {
            out.push(name.to_owned());
        }
        rest = &after[close + 2..];
    }
    Ok(out)
}

/// Wstawia parametry do szablonu (napisy wprost, inne wartości jako JSON; brak = pusty).
pub fn render(template: &str, params: &Map<String, Value>) -> Result<String, String> {
    let mut out = String::with_capacity(template.len());
    let mut rest = template;
    while let Some(open) = rest.find("{{") {
        out.push_str(&rest[..open]);
        let after = &rest[open + 2..];
        let close = after.find("}}").ok_or("niedomknięty znacznik `{{`")?;
        let name = after[..close].trim();
        match params.get(name) {
            Some(Value::String(s)) => out.push_str(s),
            Some(other) => out.push_str(&other.to_string()),
            None => {}
        }
        rest = &after[close + 2..];
    }
    out.push_str(rest);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn schema() -> Value {
        json!({
            "type": "object",
            "properties": {
                "folder": {"type": "string", "maxLength": 10},
                "dni": {"type": "integer", "minimum": 1, "maximum": 30, "default": 7},
                "tryb": {"type": "string", "enum": ["typ", "data"]},
                "maski": {"type": "array", "items": {"type": "string"}}
            },
            "required": ["folder"],
            "additionalProperties": false
        })
    }

    #[test]
    fn schema_and_params() {
        assert!(validate_schema(&schema()).is_ok());
        let ok = validate_params(&schema(), &json!({"folder": "Pobrane", "tryb": "typ"})).unwrap();
        assert_eq!(ok["dni"], 7);
        for bad in [
            json!({}),
            json!({"folder": "x".repeat(11)}),
            json!({"folder": "a", "dni": 31}),
            json!({"folder": "a", "tryb": "inny"}),
            json!({"folder": "a", "obcy": 1}),
            json!({"folder": 3}),
            json!({"folder": "a", "maski": [1]}),
            json!([1]),
        ] {
            assert!(validate_params(&schema(), &bad).is_err(), "{bad}");
        }
        for bad in [
            json!({"type": "string"}),
            json!({"type": "object", "properties": {}}),
            json!({"type": "object", "additionalProperties": false, "properties": {"A": {"type": "string"}}}),
            json!({"type": "object", "additionalProperties": false, "properties": {"a": {"type": "object"}}}),
            json!({"type": "object", "additionalProperties": false, "required": ["b"]}),
            json!({"type": "object", "additionalProperties": false, "properties": {"a": {"type": "integer", "default": "x"}}}),
        ] {
            assert!(validate_schema(&bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn templates() {
        assert_eq!(
            placeholders("A {{folder}} B {{ dni }} {{folder}}").unwrap(),
            vec!["folder", "dni"]
        );
        assert!(placeholders("{{nie domknięty").is_err());
        assert!(placeholders("{{Zła}}").is_err());
        let p = validate_params(&schema(), &json!({"folder": "Pobrane"})).unwrap();
        assert_eq!(
            render("Porządkuj {{folder}} z {{dni}} dni{{tryb}}.", &p).unwrap(),
            "Porządkuj Pobrane z 7 dni."
        );
    }
}
