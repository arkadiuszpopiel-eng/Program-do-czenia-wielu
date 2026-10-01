//! Stary format manifestu fali F5 (`evals/F5/MANIFEST.json`): ścieżki względem katalogu
//! manifestu, progi jako tekst — konwersja do formatu natywnego bez zmiany plików zestawu.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::error::EvalError;
use crate::manifest::{
    CaseFormat, CaseSource, MANIFEST_SCHEMA_VERSION, SuiteId, SuiteManifest, SuiteStatus,
    Threshold, ThresholdRule, validate_rel_path,
};

/// Stary format manifestu (fala F5): ścieżki względem katalogu manifestu, progi jako tekst.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LegacyManifest {
    /// Fala.
    pub wave: String,
    /// Wersja.
    pub version: u32,
    /// Status (tekst; `frozen…` = zamrożony).
    pub status: String,
    /// Data.
    pub created: String,
    /// Plik → SHA-256.
    pub files: BTreeMap<String, String>,
    /// ID kryterium → próg tekstowy.
    #[serde(default)]
    pub thresholds: BTreeMap<String, String>,
}

impl LegacyManifest {
    /// Konwersja do formatu natywnego; `dir` — katalog manifestu względem korzenia (np. `F5`).
    pub fn into_suite(self, dir: &str) -> Result<SuiteManifest, EvalError> {
        validate_rel_path(dir)?;
        let suite = SuiteId::new(self.wave.to_ascii_lowercase())?;
        let status = if self.status.trim_start().starts_with("frozen") {
            SuiteStatus::Frozen
        } else {
            SuiteStatus::Proposed
        };
        let files = self
            .files
            .into_iter()
            .map(|(path, hash)| (format!("{dir}/{path}"), hash))
            .collect::<BTreeMap<_, _>>();
        let cases = files
            .keys()
            .map(|path| CaseSource {
                path: path.clone(),
                format: CaseFormat::Opaque,
                split: None,
            })
            .collect();
        let thresholds = self
            .thresholds
            .into_iter()
            .map(|(id, text)| Threshold {
                id,
                rule: ThresholdRule::Text { text },
                description: String::new(),
            })
            .collect();
        let manifest = SuiteManifest {
            schema: MANIFEST_SCHEMA_VERSION,
            suite,
            wave: self.wave,
            version: self.version,
            status,
            created: self.created,
            accepted_by: None,
            description: format!("{} (stary format manifestu)", self.status),
            files,
            cases,
            thresholds,
        };
        manifest.validate()?;
        Ok(manifest)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_manifest_converts_with_prefix() {
        let legacy: LegacyManifest = serde_json::from_value(serde_json::json!({
            "wave": "F5", "version": 1, "status": "proposed — x", "created": "2026-10-01",
            "files": {"a.json": "c".repeat(64)}, "thresholds": {"F5-01": "0/100"}
        }))
        .unwrap();
        let m = legacy.into_suite("F5").unwrap();
        assert_eq!(m.suite.as_str(), "f5");
        assert_eq!(m.status, SuiteStatus::Proposed);
        assert!(m.files.contains_key("F5/a.json"));
        assert!(matches!(m.thresholds[0].rule, ThresholdRule::Text { .. }));
    }
}
