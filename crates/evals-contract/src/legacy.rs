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
    /// Podzestaw fali (np. `voice` w `evals/F5/voice/`) — identyfikator `<fala>-<set>`; bez
    /// niego dwa manifesty tej samej fali dawały ten sam zestaw (fala 5, „powtórzony zestaw f5”).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub set: Option<String>,
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
    /// Identyfikator: `<fala>` albo `<fala>-<set>` małymi literami (np. `f5`, `f5-voice`).
    pub fn into_suite(self, dir: &str) -> Result<SuiteManifest, EvalError> {
        validate_rel_path(dir)?;
        let id = match &self.set {
            Some(set) => format!("{}-{set}", self.wave),
            None => self.wave.clone(),
        };
        let suite = SuiteId::new(id.to_ascii_lowercase())?;
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

    #[test]
    fn legacy_manifest_with_set_gets_its_own_suite_id() {
        // `evals/F5/voice/MANIFEST.json` ma `set: "voice"` — bez tego dublował zestaw `f5`.
        let legacy: LegacyManifest = serde_json::from_value(serde_json::json!({
            "wave": "F5", "set": "voice", "version": 1, "status": "proposed", "created": "2026-10-02",
            "files": {"a.json": "c".repeat(64)}, "corpus": "poza gitem"
        }))
        .unwrap();
        let m = legacy.into_suite("F5/voice").unwrap();
        assert_eq!(m.suite.as_str(), "f5-voice");
        assert!(m.files.contains_key("F5/voice/a.json"));
        let bad: LegacyManifest = serde_json::from_value(serde_json::json!({
            "wave": "F5", "set": "Głos!", "version": 1, "status": "proposed", "created": "x",
            "files": {}
        }))
        .unwrap();
        assert!(bad.into_suite("F5/x").is_err());
    }
}
