//! Hash treści (SHA-256 kanonicznego JSON-a: klucze posortowane, bez białych znaków) i paczka
//! eksportu/importu `alfa.skills.v1` (w `.alfa` jako dokument kategorii `skills` przez
//! `DocumentStore`). Import weryfikuje format i hash; umiejętności z paczki trafiają do
//! biblioteki wyłącznie jako propozycje (z zewnątrz — do kwarantanny).

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::model::{Skill, SkillSource};

/// Format paczki.
pub const BUNDLE_FORMAT: &str = "alfa.skills.v1";
/// Nazwa dokumentu w kategorii `skills` paczki `.alfa`.
pub const BUNDLE_DOCUMENT: &str = "skills.json";
/// Największa paczka importu (B).
pub const MAX_BUNDLE_BYTES: usize = 4 * 1024 * 1024;

fn write_canonical(v: &Value, out: &mut String) {
    match v {
        Value::Object(m) => {
            let mut keys: Vec<&String> = m.keys().collect();
            keys.sort();
            out.push('{');
            for (i, k) in keys.into_iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                out.push_str(&Value::String(k.clone()).to_string());
                out.push(':');
                if let Some(x) = m.get(k) {
                    write_canonical(x, out);
                }
            }
            out.push('}');
        }
        Value::Array(a) => {
            out.push('[');
            for (i, x) in a.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                write_canonical(x, out);
            }
            out.push(']');
        }
        other => out.push_str(&other.to_string()),
    }
}

/// Kanoniczny JSON (niezależny od kolejności kluczy).
pub fn canonical_json<T: Serialize>(value: &T) -> Result<String, String> {
    let v = serde_json::to_value(value).map_err(|e| e.to_string())?;
    let mut out = String::new();
    write_canonical(&v, &mut out);
    Ok(out)
}

/// SHA-256 (hex) kanonicznego JSON-a.
pub fn content_hash<T: Serialize>(value: &T) -> Result<String, String> {
    let digest = Sha256::digest(canonical_json(value)?.as_bytes());
    Ok(digest.iter().map(|b| format!("{b:02x}")).collect())
}

/// Umiejętność w paczce (ze źródłem — informacyjnie; przy imporcie źródłem jest import).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BundledSkill {
    /// Przepis.
    pub skill: Skill,
    /// Źródło w bibliotece eksportującej.
    pub source: SkillSource,
}

/// Paczka umiejętności.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SkillBundle {
    /// Format ([`BUNDLE_FORMAT`]).
    pub format: String,
    /// Umiejętności.
    pub skills: Vec<BundledSkill>,
    /// SHA-256 kanonicznego JSON-a `skills`.
    pub sha256: String,
}

impl SkillBundle {
    /// Paczka z hashem.
    pub fn new(skills: Vec<BundledSkill>) -> Result<Self, String> {
        let sha256 = content_hash(&skills)?;
        Ok(Self {
            format: BUNDLE_FORMAT.to_owned(),
            skills,
            sha256,
        })
    }

    /// Bajty dokumentu (JSON).
    pub fn to_bytes(&self) -> Result<Vec<u8>, String> {
        serde_json::to_vec_pretty(self).map_err(|e| e.to_string())
    }

    /// Odczyt i weryfikacja (rozmiar, format, hash).
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() > MAX_BUNDLE_BYTES {
            return Err("paczka umiejętności za duża".into());
        }
        let b: Self = serde_json::from_slice(bytes).map_err(|e| format!("paczka: {e}"))?;
        b.verify()?;
        Ok(b)
    }

    /// Format i hash zgodne.
    pub fn verify(&self) -> Result<(), String> {
        if self.format != BUNDLE_FORMAT {
            return Err(format!("nieznany format paczki `{}`", self.format));
        }
        if content_hash(&self.skills)? != self.sha256 {
            return Err("hash paczki niezgodny — paczka zmieniona albo uszkodzona".into());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn canonical_ignores_key_order() {
        let a = json!({"b": 1, "a": {"y": [1, {"d": 2, "c": 3}], "x": "ż"}});
        let b = json!({"a": {"x": "ż", "y": [1, {"c": 3, "d": 2}]}, "b": 1});
        assert_eq!(canonical_json(&a).unwrap(), canonical_json(&b).unwrap());
        assert_eq!(
            canonical_json(&a).unwrap(),
            r#"{"a":{"x":"ż","y":[1,{"c":3,"d":2}]},"b":1}"#
        );
        assert_eq!(content_hash(&a).unwrap().len(), 64);
        assert_ne!(content_hash(&a).unwrap(), content_hash(&json!({})).unwrap());
    }
}
