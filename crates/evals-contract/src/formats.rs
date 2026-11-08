//! Formaty plików przypadków: natywny NDJSON ([`crate::EvalCase`]) i adaptery istniejących
//! zestawów F2/F3/F7 — czytane bez zmiany ich treści (ACCEPTANCE §13).

use serde_json::Value;

use crate::case::{EvalCase, validate_cases};
use crate::error::EvalError;
use crate::manifest::{CaseFormat, CaseSource, Split};

fn format_error(source: &CaseSource, detail: impl Into<String>) -> EvalError {
    EvalError::CaseFormat {
        path: source.path.clone(),
        detail: detail.into(),
    }
}

/// Linie NDJSON (puste i zaczynające się od `#` pomijane) jako wartości JSON.
fn ndjson(source: &CaseSource, bytes: &[u8]) -> Result<Vec<Value>, EvalError> {
    let text =
        std::str::from_utf8(bytes).map_err(|_| format_error(source, "plik nie jest UTF-8"))?;
    text.lines()
        .enumerate()
        .filter(|(_, l)| !l.trim().is_empty() && !l.trim_start().starts_with('#'))
        .map(|(n, l)| {
            serde_json::from_str(l)
                .map_err(|e| format_error(source, format!("linia {}: {e}", n + 1)))
        })
        .collect()
}

fn text_field(source: &CaseSource, line: &Value, field: &str) -> Result<String, EvalError> {
    line.get(field)
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| format_error(source, format!("brak pola `{field}`")))
}

/// Podział: jawny ze źródła albo z linii; przy obu muszą być zgodne.
fn split_of(source: &CaseSource, line_split: Option<Split>) -> Result<Split, EvalError> {
    match (source.split, line_split) {
        (Some(a), Some(b)) if a != b => Err(format_error(
            source,
            format!(
                "podział linii `{}` ≠ podział źródła `{}`",
                b.as_str(),
                a.as_str()
            ),
        )),
        (Some(s), _) | (None, Some(s)) => Ok(s),
        (None, None) => Ok(Split::Test),
    }
}

fn parse_split(source: &CaseSource, line: &Value) -> Result<Option<Split>, EvalError> {
    line.get("split")
        .map(|v| serde_json::from_value(v.clone()).map_err(|e| format_error(source, e.to_string())))
        .transpose()
}

/// Przypadki z pliku źródła. Format [`CaseFormat::Opaque`] nie ma przypadków.
pub fn parse_cases(source: &CaseSource, bytes: &[u8]) -> Result<Vec<EvalCase>, EvalError> {
    let cases = match source.format {
        CaseFormat::Opaque => Vec::new(),
        CaseFormat::EvalCases => ndjson(source, bytes)?
            .into_iter()
            .map(|line| {
                let case: EvalCase = serde_json::from_value(line)
                    .map_err(|e| format_error(source, e.to_string()))?;
                let split = split_of(source, Some(case.split))?;
                Ok(EvalCase { split, ..case })
            })
            .collect::<Result<_, EvalError>>()?,
        CaseFormat::F2VoiceManifest => ndjson(source, bytes)?
            .into_iter()
            .map(|line| {
                Ok(EvalCase {
                    id: text_field(source, &line, "id")?,
                    split: split_of(source, parse_split(source, &line)?)?,
                    class: line.get("kind").and_then(Value::as_str).map(str::to_owned),
                    expected: line.get("transcript").cloned().unwrap_or(Value::Null),
                    input: line,
                })
            })
            .collect::<Result<_, EvalError>>()?,
        CaseFormat::F3ToolTasks => {
            let doc: Value =
                serde_json::from_slice(bytes).map_err(|e| format_error(source, e.to_string()))?;
            let tasks = doc
                .get("tasks")
                .and_then(Value::as_array)
                .ok_or_else(|| format_error(source, "brak tablicy `tasks`"))?;
            tasks
                .iter()
                .map(|task| {
                    Ok(EvalCase {
                        id: text_field(source, task, "id")?,
                        split: split_of(source, None)?,
                        class: task.get("kind").and_then(Value::as_str).map(str::to_owned),
                        expected: task.get("expect").cloned().unwrap_or(Value::Null),
                        input: task.clone(),
                    })
                })
                .collect::<Result<_, EvalError>>()?
        }
        CaseFormat::F7RecallQueries => ndjson(source, bytes)?
            .into_iter()
            .map(|line| {
                Ok(EvalCase {
                    id: text_field(source, &line, "id")?,
                    split: split_of(source, None)?,
                    class: line.get("kind").and_then(Value::as_str).map(str::to_owned),
                    expected: line.get("expected").cloned().unwrap_or(Value::Null),
                    input: line,
                })
            })
            .collect::<Result<_, EvalError>>()?,
    };
    validate_cases(&cases).map_err(|e| format_error(source, e.to_string()))?;
    Ok(cases)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn src(format: CaseFormat, split: Option<Split>) -> CaseSource {
        CaseSource {
            path: "x".into(),
            format,
            split,
        }
    }

    #[test]
    fn adapters_read_existing_formats() {
        let f2 = br#"# komentarz
{"id":"a","split":"dev","kind":"command","transcript":"stop"}

{"id":"b","split":"test","kind":"free_speech"}
"#;
        let c = parse_cases(&src(CaseFormat::F2VoiceManifest, None), f2).unwrap();
        assert_eq!(c.len(), 2);
        assert_eq!(
            (c[0].split, c[0].class.as_deref()),
            (Split::Dev, Some("command"))
        );
        assert_eq!(c[0].expected, "stop");
        assert!(parse_cases(&src(CaseFormat::F2VoiceManifest, Some(Split::Holdout)), f2).is_err());

        let f3 = br#"{"version":1,"tasks":[{"id":"fs-01","kind":"fs","expect":{"files":{}}}]}"#;
        let c = parse_cases(&src(CaseFormat::F3ToolTasks, None), f3).unwrap();
        assert_eq!(
            (c[0].split, c[0].class.as_deref()),
            (Split::Test, Some("fs"))
        );

        let f7 = br#"{"id":"q1","query":"x","expected":["f1"],"kind":"pytanie"}"#;
        let c = parse_cases(&src(CaseFormat::F7RecallQueries, Some(Split::Holdout)), f7).unwrap();
        assert_eq!(c[0].split, Split::Holdout);
        assert_eq!(c[0].expected, serde_json::json!(["f1"]));
    }

    #[test]
    fn native_and_errors() {
        let ok = br#"{"id":"a","split":"test"}"#;
        assert_eq!(
            parse_cases(&src(CaseFormat::EvalCases, Some(Split::Test)), ok)
                .unwrap()
                .len(),
            1
        );
        assert!(parse_cases(&src(CaseFormat::EvalCases, Some(Split::Dev)), ok).is_err());
        let dup = b"{\"id\":\"a\",\"split\":\"test\"}\n{\"id\":\"a\",\"split\":\"test\"}";
        assert!(parse_cases(&src(CaseFormat::EvalCases, None), dup).is_err());
        assert!(parse_cases(&src(CaseFormat::EvalCases, None), b"{zly json").is_err());
        assert!(parse_cases(&src(CaseFormat::EvalCases, None), &[0xff, 0xfe]).is_err());
        assert!(parse_cases(&src(CaseFormat::F3ToolTasks, None), b"{}").is_err());
        assert!(
            parse_cases(&src(CaseFormat::Opaque, None), b"cokolwiek")
                .unwrap()
                .is_empty()
        );
    }
}
