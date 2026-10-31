//! Wyjście sondy częściami: `probe.ps1` wypisuje każdą część od razu jako jeden wiersz JSON
//! (`{"k":"cpu","v":…}`; błąd części — `{"k":"gap","v":{"part":…,"error":…}}`). Gdy któraś część
//! wisi ponad limit (31.10: karty sieciowe > 80 s), sonda zostaje zatrzymana, a zapis powstaje
//! z części odczytanych do tej pory — brakujące trafiają do luk zamiast unieważniać cały odczyt.

use serde_json::{Map, Value};

/// Klucze części w zapisie i nazwy części w lukach — w kolejności skryptu.
pub const PARTS: [(&str, &str); 11] = [
    ("system", "system"),
    ("cpu", "cpu"),
    ("modules", "memory_modules"),
    ("pagefile", "pagefile"),
    ("disks", "disks"),
    ("volumes", "volumes"),
    ("gpus", "gpus"),
    ("net", "network"),
    ("battery", "battery"),
    ("thermal", "thermal"),
    ("events", "events"),
];

/// Składa wiersze sondy w jeden obiekt JSON dla [`crate::parse::parse`]. `timeout_s` — gdy sonda
/// została zatrzymana po limicie: części bez odpowiedzi stają się lukami. Bez żadnej części — błąd.
pub fn assemble(out: &str, timeout_s: Option<u64>) -> Result<String, String> {
    let mut parts = Map::new();
    let mut gaps = Vec::new();
    for line in out.lines().map(|l| l.trim_start_matches('\u{feff}').trim()) {
        let Ok(Value::Object(mut row)) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        let (Some(Value::String(key)), value) = (row.remove("k"), row.remove("v")) else {
            continue;
        };
        match key.as_str() {
            "gap" => gaps.extend(value),
            _ => {
                parts.insert(key, value.unwrap_or(Value::Null));
            }
        }
    }
    if parts.is_empty() {
        return Err(match timeout_s {
            Some(s) => format!("odczyt trwał dłużej niż {s} s"),
            None => "sonda nie zwróciła żadnej części".into(),
        });
    }
    if let Some(s) = timeout_s {
        for (key, part) in PARTS {
            if !parts.contains_key(key) {
                gaps.push(serde_json::json!({
                    "part": part,
                    "error": format!("nie zdążyło w limicie {s} s — sonda zatrzymana"),
                }));
            }
        }
    }
    parts.insert("gaps".into(), Value::Array(gaps));
    Ok(Value::Object(parts).to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rows_become_one_object_with_gaps() {
        let out = "\u{feff}{\"k\":\"system\",\"v\":{\"name\":\"PC\"}}\r\n\
                   {\"k\":\"gap\",\"v\":{\"part\":\"thermal\",\"error\":\"Odmowa\"}}\r\n\
                   śmieci z ostrzeżenia\r\n{\"k\":\"thermal\",\"v\":null}\r\n";
        let v: Value = serde_json::from_str(&assemble(out, None).unwrap()).unwrap();
        assert_eq!(v["system"]["name"], "PC");
        assert!(v["thermal"].is_null());
        assert_eq!(v["gaps"].as_array().unwrap().len(), 1);
        assert_eq!(v["gaps"][0]["part"], "thermal");
    }

    #[test]
    fn stopped_probe_keeps_read_parts_and_names_the_missing_ones() {
        let out = "{\"k\":\"system\",\"v\":{}}\n{\"k\":\"cpu\",\"v\":[]}\n";
        let v: Value = serde_json::from_str(&assemble(out, Some(90)).unwrap()).unwrap();
        let missing: Vec<&str> = v["gaps"]
            .as_array()
            .unwrap()
            .iter()
            .map(|g| g["part"].as_str().unwrap())
            .collect();
        assert_eq!(missing.len(), PARTS.len() - 2);
        assert!(missing.contains(&"network") && missing.contains(&"events"));
        assert!(v["gaps"][0]["error"].as_str().unwrap().contains("90 s"));
    }

    #[test]
    fn nothing_read_is_an_error() {
        assert_eq!(
            assemble("", Some(90)).unwrap_err(),
            "odczyt trwał dłużej niż 90 s"
        );
        assert!(assemble("nie JSON", None).is_err());
    }
}
