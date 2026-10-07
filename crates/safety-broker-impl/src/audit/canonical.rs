//! Kanoniczny JSON i SHA-256 dla łańcucha Audytu (ta sama postać co łańcuch `pre-broker`
//! w `core-log-impl` — bez zależności od cudzego `-impl`): bez białych znaków, klucze obiektów
//! posortowane bajtowo, liczby w najkrótszym zapisie odtwarzalnym.

use serde_json::Value;
use sha2::{Digest, Sha256};

/// Kanoniczny zapis wartości JSON (funkcja różnowartościowa).
pub fn canonical_json(value: &Value) -> String {
    let mut out = String::new();
    write_value(value, &mut out);
    out
}

fn write_value(value: &Value, out: &mut String) {
    match value {
        Value::Object(map) => {
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort();
            out.push('{');
            for (i, key) in keys.into_iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                out.push_str(&Value::String(key.clone()).to_string());
                out.push(':');
                if let Some(v) = map.get(key) {
                    write_value(v, out);
                }
            }
            out.push('}');
        }
        Value::Array(items) => {
            out.push('[');
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                write_value(item, out);
            }
            out.push(']');
        }
        scalar => out.push_str(&scalar.to_string()),
    }
}

/// SHA-256 jako 64 znaki hex.
pub fn sha256_hex(data: &[u8]) -> String {
    safety_broker_contract::hex::encode(&Sha256::digest(data))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn canonical_and_hash() {
        let v = json!({"b": [1, {"z": null, "a": true}], "a": "x\"y", "ą": 1.5});
        assert_eq!(
            canonical_json(&v),
            r#"{"a":"x\"y","b":[1,{"a":true,"z":null}],"ą":1.5}"#
        );
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }
}
