//! Odcisk (hash) definicji narzędzia: SHA-256 kanonicznego JSON. Zgoda użytkownika wiąże się
//! z odciskiem; zmiana opisu, schematu albo adnotacji po zatwierdzeniu = nowy odcisk = blokada
//! do ponownej zgody (PLAN §8.7, THREAT_MODEL S07/S08).

use std::fmt;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::protocol::Tool;

/// Odcisk narzędzia (`sha256:` + 64 znaki hex).
#[derive(
    Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(transparent)]
pub struct ToolFingerprint(String);

impl ToolFingerprint {
    /// Tekst odcisku.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Odtwarza odcisk z tekstu (np. z zapisanej zgody); `None`, gdy format jest zły.
    pub fn parse(text: &str) -> Option<Self> {
        let hex = text.strip_prefix("sha256:")?;
        (hex.len() == 64
            && hex
                .chars()
                .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()))
        .then(|| Self(text.to_owned()))
    }
}

impl fmt::Display for ToolFingerprint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Kanoniczny JSON: klucze obiektów posortowane rekurencyjnie, bez białych znaków.
/// Niezależny od cechy `preserve_order` serde_json (unifikacja cech w workspace).
pub fn canonical_json(value: &Value) -> String {
    let mut out = String::new();
    write_canonical(value, &mut out);
    out
}

fn write_canonical(value: &Value, out: &mut String) {
    match value {
        Value::Object(map) => {
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort();
            out.push('{');
            for (i, key) in keys.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                out.push_str(&Value::String((*key).clone()).to_string());
                out.push(':');
                if let Some(v) = map.get(*key) {
                    write_canonical(v, out);
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
                write_canonical(item, out);
            }
            out.push(']');
        }
        scalar => out.push_str(&scalar.to_string()),
    }
}

/// Odcisk definicji narzędzia (nazwa, tytuł, opis, schematy, adnotacje).
pub fn fingerprint(tool: &Tool) -> ToolFingerprint {
    let doc = serde_json::json!({
        "name": tool.name,
        "title": tool.title,
        "description": tool.description,
        "inputSchema": tool.input_schema,
        "outputSchema": tool.output_schema,
        "annotations": tool.annotations,
    });
    let digest = Sha256::digest(canonical_json(&doc).as_bytes());
    let mut hex = String::with_capacity(64 + 7);
    hex.push_str("sha256:");
    for byte in digest {
        hex.push(char::from(HEX[usize::from(byte >> 4)]));
        hex.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    ToolFingerprint(hex)
}

const HEX: &[u8; 16] = b"0123456789abcdef";

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn tool(description: &str, schema: Value) -> Tool {
        Tool {
            name: "read".into(),
            title: None,
            description: Some(description.into()),
            input_schema: schema,
            output_schema: None,
            annotations: None,
        }
    }

    #[test]
    fn canonical_json_sorts_keys() {
        let a = json!({"b": 1, "a": {"d": [1, {"z": 0, "y": 1}], "c": "x"}});
        assert_eq!(
            canonical_json(&a),
            r#"{"a":{"c":"x","d":[1,{"y":1,"z":0}]},"b":1}"#
        );
    }

    #[test]
    fn fingerprint_is_stable_and_sensitive() {
        let base = tool(
            "Czyta plik",
            json!({"type": "object", "properties": {"p": {"type": "string"}}}),
        );
        let reordered = tool(
            "Czyta plik",
            json!({"properties": {"p": {"type": "string"}}, "type": "object"}),
        );
        assert_eq!(fingerprint(&base), fingerprint(&reordered));
        assert_ne!(
            fingerprint(&base),
            fingerprint(&tool("Czyta plik.", base.input_schema.clone()))
        );
        let mut annotated = base.clone();
        annotated.annotations = Some(json!({"readOnlyHint": true}));
        assert_ne!(fingerprint(&base), fingerprint(&annotated));
        let fp = fingerprint(&base);
        assert_eq!(ToolFingerprint::parse(fp.as_str()), Some(fp.clone()));
        assert!(fp.to_string().starts_with("sha256:"));
        assert_eq!(ToolFingerprint::parse("sha256:zz"), None);
        assert_eq!(ToolFingerprint::parse(&fp.as_str().to_uppercase()), None);
    }
}
