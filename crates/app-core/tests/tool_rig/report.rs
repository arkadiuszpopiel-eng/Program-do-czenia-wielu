//! Raport diagnostyki narzędzi: wynik każdego wywołania (klasa wyniku, czas, ślad na atrapach,
//! zgodność danych z `output_schema`) w `target/verify/narzedzia/raport.{json,md}` i porównanie
//! z wzorcem `tests/tool_rig/wzorzec.json` (narzędzie → oczekiwana klasa wyniku).

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::Serialize;
use serde_json::Value;
use tools_common_contract::{ToolErrorKind, ToolStatus};

/// Wynik jednego narzędzia.
#[derive(Debug, Clone, Serialize)]
pub struct Entry {
    pub name: String,
    pub id: String,
    pub schema: Value,
    pub groups: Vec<String>,
    pub mutating: bool,
    pub source: &'static str,
    pub args: Value,
    /// Klasa wyniku: `ok`, `odmowa:<powód>`, `potwierdzenie`, `blad:<rodzaj>`, `anulowano`,
    /// `brak-odpowiedzi`, `panika`.
    pub class: String,
    pub ms: u128,
    pub text: String,
    /// Błąd zgodności danych z `output_schema` (`None` — zgodne albo bez danych).
    pub output_error: Option<String>,
    /// Błąd zgodności argumentów (generator + przepis) z `input_schema`.
    pub args_error: Option<String>,
    /// Ślad na atrapach: polecenia (`exec`), zmienione pliki (`fs`).
    pub trace: String,
}

/// Klasa wyniku ze statusu.
pub fn class(status: &ToolStatus) -> String {
    match status {
        ToolStatus::Ok => "ok".into(),
        ToolStatus::Denied { reason } => {
            let kind = serde_json::to_value(reason)
                .ok()
                .and_then(|v| v["reason"].as_str().map(str::to_owned))
                .unwrap_or_else(|| "nieznany".into());
            format!("odmowa:{kind}")
        }
        ToolStatus::NeedsConfirmation => "potwierdzenie".into(),
        ToolStatus::Failed { error } => format!("blad:{}", error_kind(*error)),
        ToolStatus::Cancelled => "anulowano".into(),
    }
}

fn error_kind(kind: ToolErrorKind) -> &'static str {
    match kind {
        ToolErrorKind::InvalidArgs => "invalid_args",
        ToolErrorKind::NotFound => "not_found",
        ToolErrorKind::AlreadyExists => "already_exists",
        ToolErrorKind::Io => "io",
        ToolErrorKind::Unsupported => "unsupported",
        ToolErrorKind::Timeout => "timeout",
        ToolErrorKind::Internal => "internal",
    }
}

/// Usterki wpisu niezależne od wzorca (każda to czerwony test).
pub fn defects(e: &Entry) -> Vec<String> {
    let mut out = Vec::new();
    match e.class.as_str() {
        "blad:invalid_args" if super::args::known_rejection(&e.name).is_some() => {}
        "blad:invalid_args" => out.push(format!(
            "poprawne argumenty ({}) odrzucone jako błędne — popraw przepis albo schemat",
            e.source
        )),
        "blad:internal" => out.push("błąd wewnętrzny (Sędzia Cieni, dziennik cofania)".into()),
        "brak-odpowiedzi" | "panika" => out.push(e.class.clone()),
        _ => {}
    }
    if e.text.trim().is_empty() {
        out.push("pusty tekst wyniku dla modelu".into());
    }
    if let Some(err) = &e.args_error {
        out.push(format!(
            "argumenty ({}) niezgodne z input_schema: {err}",
            e.source
        ));
    }
    if let Some(err) = &e.output_error {
        out.push(format!("dane niezgodne z output_schema: {err}"));
    }
    out
}

/// Błąd zgodności `data` ze schematem (pusty schemat albo `data: null` — bez sprawdzania).
pub fn check_output(schema: &Value, data: &Value) -> Option<String> {
    if data.is_null() || schema.as_object().is_none_or(serde_json::Map::is_empty) {
        return None;
    }
    match jsonschema::validator_for(schema) {
        Ok(v) => v
            .iter_errors(data)
            .next()
            .map(|e| format!("{} ({})", e, e.instance_path())),
        Err(e) => Some(format!("niepoprawny output_schema: {e}")),
    }
}

/// Katalog raportu (`target/verify/narzedzia`).
pub fn out_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/verify/narzedzia")
}

/// Wzorzec klas wyników (plik w repozytorium).
pub fn baseline_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/tool_rig/wzorzec.json")
}

pub fn read_baseline() -> BTreeMap<String, String> {
    std::fs::read_to_string(baseline_path())
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default()
}

pub fn write_baseline(entries: &[Entry]) {
    let map: BTreeMap<&str, &str> = entries
        .iter()
        .filter(|e| e.source != "straż")
        .map(|e| (e.name.as_str(), e.class.as_str()))
        .collect();
    let text = serde_json::to_string_pretty(&map).unwrap() + "\n";
    std::fs::write(baseline_path(), text).unwrap();
}

/// Zapis raportu JSON i tabeli Markdown.
pub fn write(entries: &[Entry], problems: &[String]) {
    let dir = out_dir();
    std::fs::create_dir_all(&dir).unwrap();
    let json = serde_json::json!({ "narzedzia": entries, "problemy": problems });
    std::fs::write(
        dir.join("raport.json"),
        serde_json::to_string_pretty(&json).unwrap(),
    )
    .unwrap();
    let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
    let (probes, tools): (Vec<&Entry>, Vec<&Entry>) =
        entries.iter().partition(|e| e.source == "straż");
    for e in &tools {
        *counts
            .entry(e.class.split(':').next().unwrap_or(""))
            .or_default() += 1;
    }
    let mut md = format!(
        "# Diagnostyka narzędzi agentek\n\nNarzędzi: {}. Klasy: {}. Próby straży: {}. Problemy: {}.\n\n",
        tools.len(),
        counts
            .iter()
            .map(|(k, v)| format!("{k} {v}"))
            .collect::<Vec<_>>()
            .join(", "),
        probes.len(),
        problems.len()
    );
    for p in problems {
        md.push_str(&format!("- **{p}**\n"));
    }
    let notes: Vec<String> = entries
        .iter()
        .filter(|e| e.class != "ok" && e.source != "straż")
        .filter_map(|e| {
            super::args::note(&e.name).map(|n| format!("- `{}` ({}): {n}\n", e.name, e.class))
        })
        .collect();
    if !notes.is_empty() {
        md.push_str("\n## Wyniki inne niż ok — dlaczego\n\n");
        md.push_str(&notes.concat());
    }
    md.push_str(
        "\n| Narzędzie | Wynik | ms | Argumenty | Ślad | Tekst |\n|---|---|---|---|---|---|\n",
    );
    for e in entries {
        let text: String = e.text.chars().take(120).collect();
        md.push_str(&format!(
            "| `{}` | {} | {} | {} `{}` | {} | {} |\n",
            e.name,
            e.class,
            e.ms,
            e.source,
            e.args,
            e.trace,
            text.replace('|', "\\|").replace('\n', " ")
        ));
    }
    std::fs::write(dir.join("raport.md"), md).unwrap();
}
