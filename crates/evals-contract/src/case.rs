//! Przypadek ewaluacyjny i wynik jego przebiegu.

use std::collections::{BTreeMap, BTreeSet};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::EvalError;
use crate::manifest::Split;

/// Przypadek (linia NDJSON w formacie natywnym).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct EvalCase {
    /// Identyfikator unikalny w zestawie.
    pub id: String,
    /// Podział.
    pub split: Split,
    /// Klasa (próg per klasa), np. rodzaj intencji.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub class: Option<String>,
    /// Wejście (dowolny JSON; interpretuje go uruchamiający wariant).
    #[serde(default)]
    pub input: Value,
    /// Oczekiwany wynik.
    #[serde(default)]
    pub expected: Value,
}

/// Wynik jednego powtórzenia przypadku.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct CaseOutcome {
    /// Przypadek.
    pub case_id: String,
    /// Klasa przypadku.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub class: Option<String>,
    /// Numer powtórzenia (od 0).
    pub repeat: u32,
    /// Czy zaliczony.
    pub passed: bool,
    /// Metryki dodatkowe (np. `latency_ms`, `wer`).
    #[serde(default)]
    pub metrics: BTreeMap<String, f64>,
}

impl CaseOutcome {
    /// Wynik dla przypadku.
    pub fn new(case: &EvalCase, repeat: u32, passed: bool) -> Self {
        Self {
            case_id: case.id.clone(),
            class: case.class.clone(),
            repeat,
            passed,
            metrics: BTreeMap::new(),
        }
    }

    /// Dodaje metrykę (builder); wartości nieskończone są pomijane.
    #[must_use]
    pub fn with_metric(mut self, name: &str, value: f64) -> Self {
        if value.is_finite() {
            self.metrics.insert(name.to_owned(), value);
        }
        self
    }
}

/// Unikalne, niepuste identyfikatory przypadków.
pub fn validate_cases(cases: &[EvalCase]) -> Result<(), EvalError> {
    let mut seen = BTreeSet::new();
    for case in cases {
        if case.id.trim().is_empty() || !seen.insert(case.id.as_str()) {
            return Err(EvalError::CaseFormat {
                path: String::new(),
                detail: format!(
                    "pusty albo powtórzony identyfikator przypadku `{}`",
                    case.id
                ),
            });
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn outcome_builder_and_validation() {
        let case = EvalCase {
            id: "c1".into(),
            split: Split::Test,
            class: Some("k".into()),
            input: Value::Null,
            expected: Value::Null,
        };
        let o = CaseOutcome::new(&case, 2, true)
            .with_metric("x", 1.5)
            .with_metric("nan", f64::NAN);
        assert_eq!(o.metrics.len(), 1);
        assert_eq!(o.class.as_deref(), Some("k"));
        assert!(validate_cases(std::slice::from_ref(&case)).is_ok());
        assert!(validate_cases(&[case.clone(), case]).is_err());
        let line = r#"{"id":"a","split":"dev"}"#;
        let parsed: EvalCase = serde_json::from_str(line).unwrap();
        assert_eq!(parsed.split, Split::Dev);
    }
}
