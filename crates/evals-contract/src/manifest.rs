//! Manifest zestawu ewaluacyjnego (ACCEPTANCE §1: hashe SHA-256 wszystkich plików i progi).
//!
//! Ścieżki w `files` i `cases` są względne wobec **korzenia magazynu** (`evals/` dla zestawów
//! w gicie, `evals/holdout/` dla holdoutu), zapisane z `/`, bez `..`, `\`, `:` i ścieżek
//! bezwzględnych. Stary format fali F5 (`evals/F5/MANIFEST.json`, ścieżki względem katalogu
//! manifestu, progi jako tekst) czyta [`LegacyManifest`].

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::error::EvalError;
use crate::integrity::{is_sha256_hex, sha256_hex};

/// Wersja schematu manifestu.
pub const MANIFEST_SCHEMA_VERSION: u32 = 1;

/// Identyfikator zestawu: `[a-z0-9][a-z0-9._-]{0,63}` (np. `f8-chaos`).
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, JsonSchema)]
#[serde(transparent)]
pub struct SuiteId(String);

impl SuiteId {
    /// Tworzy identyfikator z walidacją.
    pub fn new(id: impl Into<String>) -> Result<Self, EvalError> {
        let id = id.into();
        let ok = (1..=64).contains(&id.len())
            && id
                .chars()
                .next()
                .is_some_and(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
            && id.chars().all(|c| {
                c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '-' | '_' | '.')
            });
        if ok {
            Ok(Self(id))
        } else {
            Err(EvalError::InvalidSuiteId(id))
        }
    }

    /// Widok tekstowy.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl<'de> Deserialize<'de> for SuiteId {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        Self::new(String::deserialize(d)?).map_err(serde::de::Error::custom)
    }
}

impl fmt::Display for SuiteId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Podział zestawu.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum Split {
    /// Rozwojowy — wolno go oglądać i stroić (także Ulepszaczowi).
    Dev,
    /// Testowy — porównanie przed/po w piaskownicy.
    Test,
    /// Ukryty — tylko przez bramkę, wynik zbiorczy (poza gitem).
    Holdout,
}

impl Split {
    /// Wszystkie podziały.
    pub const ALL: [Split; 3] = [Split::Dev, Split::Test, Split::Holdout];

    /// Nazwa jak w JSON.
    pub fn as_str(self) -> &'static str {
        match self {
            Split::Dev => "dev",
            Split::Test => "test",
            Split::Holdout => "holdout",
        }
    }
}

/// Status zestawu (ACCEPTANCE §1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SuiteStatus {
    /// Propozycja autora — rozjazd hashy raportowany, nie blokuje.
    Proposed,
    /// Zamrożony po akceptacji człowieka — każda zmiana pliku = błąd.
    Frozen,
    /// Wycofany (tylko historia).
    Retired,
}

/// Format pliku przypadków (adaptery istniejących zestawów bez zmiany ich treści).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum CaseFormat {
    /// NDJSON z liniami [`crate::EvalCase`] (format natywny).
    EvalCases,
    /// F2: `manifest.ndjson` zestawu głosu (`id`, `split`, `kind`).
    F2VoiceManifest,
    /// F3: `tasks.json` evalu narzędzi (`tasks[].id`, `kind`).
    F3ToolTasks,
    /// F7: `queries.ndjson` recall@5 (`id`, `kind`, `expected`).
    F7RecallQueries,
    /// Plik hashowany, bez przypadków (np. generator z ziarnem, F5).
    Opaque,
}

/// Źródło przypadków.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CaseSource {
    /// Plik (musi występować w `files`).
    pub path: String,
    /// Format.
    pub format: CaseFormat,
    /// Podział wszystkich przypadków pliku; `None` = z pola `split` każdej linii.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub split: Option<Split>,
}

/// Kierunek porównania progu.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Comparison {
    /// Wartość ≥ próg (sprawdzana dolna granica przedziału).
    Ge,
    /// Wartość ≤ próg (sprawdzana górna granica przedziału).
    Le,
}

/// Reguła progu.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "rule", rename_all = "snake_case")]
pub enum ThresholdRule {
    /// Próg metryki; domyślnie na granicy przedziału ufności (ACCEPTANCE §1: „dolna granica”).
    Metric {
        /// Nazwa metryki (`pass_rate` albo metryka z [`crate::CaseOutcome::metrics`]).
        metric: String,
        /// Kierunek.
        op: Comparison,
        /// Wartość progu.
        value: f64,
        /// Próg obowiązuje dla każdej klasy osobno (ACCEPTANCE §1 „Kryteria per klasa”).
        #[serde(default)]
        per_class: bool,
        /// Porównuj wartość punktową zamiast granicy przedziału.
        #[serde(default)]
        point: bool,
    },
    /// Próg opisowy (stary format; sprawdza go test modułu albo człowiek).
    Text {
        /// Treść progu.
        text: String,
    },
}

/// Próg akceptacyjny.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Threshold {
    /// ID kryterium z ACCEPTANCE (np. `F8-01`).
    pub id: String,
    /// Reguła.
    #[serde(flatten)]
    pub rule: ThresholdRule,
    /// Opis po polsku.
    #[serde(default)]
    pub description: String,
}

/// Manifest zestawu.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SuiteManifest {
    /// Wersja schematu ([`MANIFEST_SCHEMA_VERSION`]).
    pub schema: u32,
    /// Identyfikator.
    pub suite: SuiteId,
    /// Fala (np. `F8`).
    pub wave: String,
    /// Wersja zestawu (rośnie przy każdej zmianie po zamrożeniu).
    pub version: u32,
    /// Status.
    pub status: SuiteStatus,
    /// Data utworzenia (`RRRR-MM-DD`).
    pub created: String,
    /// Kto zaakceptował (tylko `frozen`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub accepted_by: Option<String>,
    /// Opis.
    #[serde(default)]
    pub description: String,
    /// Plik → SHA-256 (hex, małe litery).
    pub files: BTreeMap<String, String>,
    /// Źródła przypadków.
    #[serde(default)]
    pub cases: Vec<CaseSource>,
    /// Progi.
    #[serde(default)]
    pub thresholds: Vec<Threshold>,
}

impl SuiteManifest {
    /// Walidacja formatu (bez I/O).
    pub fn validate(&self) -> Result<(), EvalError> {
        let bad = |m: String| Err(EvalError::InvalidManifest(m));
        if self.schema != MANIFEST_SCHEMA_VERSION {
            return bad(format!("nieobsługiwana wersja schematu {}", self.schema));
        }
        if self.wave.trim().is_empty() || self.version == 0 || self.created.trim().is_empty() {
            return bad("puste `wave`/`created` albo `version` = 0".into());
        }
        if self.files.is_empty() {
            return bad("manifest bez plików".into());
        }
        for (path, hash) in &self.files {
            validate_rel_path(path)?;
            if !is_sha256_hex(hash) {
                return bad(format!("hash pliku `{path}` nie jest SHA-256 hex"));
            }
        }
        for source in &self.cases {
            if !self.files.contains_key(&source.path) {
                return bad(format!("źródło przypadków `{}` poza `files`", source.path));
            }
        }
        let mut ids = BTreeSet::new();
        for t in &self.thresholds {
            if t.id.trim().is_empty() || !ids.insert(t.id.as_str()) {
                return bad(format!(
                    "pusty albo powtórzony identyfikator progu `{}`",
                    t.id
                ));
            }
            if let ThresholdRule::Metric { metric, value, .. } = &t.rule
                && (metric.trim().is_empty() || !value.is_finite())
            {
                return bad(format!(
                    "próg `{}`: pusta metryka albo wartość nieskończona",
                    t.id
                ));
            }
        }
        Ok(())
    }

    /// Walidacja zestawu publicznego (w gicie): bez źródeł holdoutu.
    pub fn validate_public(&self) -> Result<(), EvalError> {
        self.validate()?;
        if self.cases.iter().any(|c| c.split == Some(Split::Holdout)) {
            return Err(EvalError::HoldoutInPublicSuite(self.suite.to_string()));
        }
        Ok(())
    }

    /// Walidacja zestawu holdout: każde źródło ma jawny podział `holdout`.
    pub fn validate_holdout(&self) -> Result<(), EvalError> {
        self.validate()?;
        if self.cases.is_empty() || self.cases.iter().any(|c| c.split != Some(Split::Holdout)) {
            return Err(EvalError::InvalidManifest(
                "holdout: każde źródło przypadków musi mieć `split = holdout`".into(),
            ));
        }
        Ok(())
    }

    /// SHA-256 kanonicznego JSON manifestu (do Issue fali i CI).
    pub fn digest(&self) -> String {
        sha256_hex(&serde_json::to_vec(self).unwrap_or_default())
    }
}

/// Sprawdza ścieżkę względną manifestu.
pub fn validate_rel_path(path: &str) -> Result<(), EvalError> {
    let unsafe_path = || Err(EvalError::UnsafePath(path.to_owned()));
    if path.is_empty() || path.len() > 512 || path.starts_with('/') {
        return unsafe_path();
    }
    if path
        .chars()
        .any(|c| c == '\\' || c == ':' || c.is_control())
    {
        return unsafe_path();
    }
    if path
        .split('/')
        .any(|seg| seg.is_empty() || seg == "." || seg == "..")
    {
        return unsafe_path();
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manifest() -> SuiteManifest {
        SuiteManifest {
            schema: 1,
            suite: SuiteId::new("f8-chaos").unwrap(),
            wave: "F8".into(),
            version: 1,
            status: SuiteStatus::Proposed,
            created: "2026-10-01".into(),
            accepted_by: None,
            description: String::new(),
            files: BTreeMap::from([("F8/chaos/catalog.json".into(), "a".repeat(64))]),
            cases: vec![CaseSource {
                path: "F8/chaos/catalog.json".into(),
                format: CaseFormat::Opaque,
                split: None,
            }],
            thresholds: vec![Threshold {
                id: "F8-01".into(),
                rule: ThresholdRule::Metric {
                    metric: "pass_rate".into(),
                    op: Comparison::Ge,
                    value: 1.0,
                    per_class: true,
                    point: true,
                },
                description: String::new(),
            }],
        }
    }

    #[test]
    fn suite_ids_and_paths() {
        assert!(SuiteId::new("f2-voice.v1").is_ok());
        for bad in ["", "F8", "-x", "a b", &"x".repeat(65)] {
            assert!(SuiteId::new(bad).is_err(), "{bad}");
        }
        assert!(validate_rel_path("F2/samples/manifest.ndjson").is_ok());
        for bad in [
            "", "/etc/x", "a/../b", "a\\b", "c:x", "a//b", "./a", "a/\u{0}",
        ] {
            assert!(validate_rel_path(bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn manifest_validation_and_digest() {
        let m = manifest();
        m.validate_public().unwrap();
        assert_eq!(m.digest(), manifest().digest());
        let json = serde_json::to_value(&m).unwrap();
        assert_eq!(json["thresholds"][0]["rule"], "metric");
        let back: SuiteManifest = serde_json::from_value(json).unwrap();
        assert_eq!(back, m);

        let mut holdout = m.clone();
        holdout.cases[0].split = Some(Split::Holdout);
        assert!(matches!(
            holdout.validate_public(),
            Err(EvalError::HoldoutInPublicSuite(_))
        ));
        holdout.validate_holdout().unwrap();
        assert!(m.validate_holdout().is_err());

        let mut bad = m.clone();
        bad.files.insert("F8/../x".into(), "b".repeat(64));
        assert!(matches!(bad.validate(), Err(EvalError::UnsafePath(_))));
        let mut bad = m.clone();
        bad.files.insert("F8/y".into(), "XYZ".into());
        assert!(bad.validate().is_err());
        let mut bad = m;
        bad.thresholds.push(bad.thresholds[0].clone());
        assert!(bad.validate().is_err());
    }
}
