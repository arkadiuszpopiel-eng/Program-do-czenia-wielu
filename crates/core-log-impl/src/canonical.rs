//! Kanoniczny JSON i SHA-256 dla łańcucha audytu.
//!
//! Postać kanoniczna: bez białych znaków, klucze obiektów posortowane bajtowo (UTF-8),
//! napisy i liczby w zapisie `serde_json` (liczby zmiennoprzecinkowe: najkrótszy zapis
//! odtwarzalny — stąd feature `float_roundtrip`). Funkcja jest różnowartościowa: różne wartości
//! dają różne teksty, więc `canonical(parse(linia)) == linia` wykrywa każdą zmianę formy.

use serde_json::Value;
use sha2::{Digest, Sha256};

/// Kanoniczny zapis wartości JSON.
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

/// SHA-256 jako 64 znaki hex (małe litery).
pub fn sha256_hex(data: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let digest = Sha256::digest(data);
    let mut out = String::with_capacity(64);
    for byte in digest {
        out.push(char::from(HEX[usize::from(byte >> 4)]));
        out.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn keys_sorted_no_whitespace() {
        let v = json!({"b": [1, {"z": null, "a": true}], "a": "x\"y", "ą": 1.5});
        assert_eq!(
            canonical_json(&v),
            r#"{"a":"x\"y","b":[1,{"a":true,"z":null}],"ą":1.5}"#
        );
    }

    #[test]
    fn floats_round_trip_exactly() {
        for f in [0.1, 1e-300, 123_456.789_012_345, -0.0, f64::MAX, 5e-324] {
            let text = canonical_json(&json!(f));
            let back: Value = serde_json::from_str(&text).unwrap();
            assert_eq!(canonical_json(&back), text);
        }
    }

    #[test]
    fn sha256_known_vector() {
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }
}
