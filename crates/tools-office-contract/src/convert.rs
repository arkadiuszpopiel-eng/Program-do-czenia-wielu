//! Argumenty narzędzi → zapytania i edycje portu Office (te same reguły w `-impl` i `-fake`),
//! ścieżka nowej wersji dokumentu i walidacja argumentów.

use platform_apps_contract::{
    CellInput, OfficeApp, OfficeEdit, OfficeQuery, TextPosition, app_for_file, parse_range,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{EditArgs, EditOp, PositionArg, ReadArgs, ReadWhat};

/// Wartość komórki od modelu: liczba, `true/false`, napis (od `=` — formuła; od `'` — tekst
/// dosłowny) albo `null` (wyczyszczenie).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum CellArg {
    /// Liczba.
    Number(f64),
    /// Prawda/fałsz.
    Bool(bool),
    /// Napis albo formuła.
    Text(String),
    /// Wyczyszczenie.
    Null,
}

impl CellArg {
    fn input(&self) -> CellInput {
        match self {
            Self::Number(n) => CellInput::Number(*n),
            Self::Bool(b) => CellInput::Bool(*b),
            Self::Text(t) if t.starts_with('=') => CellInput::Formula(t.clone()),
            Self::Text(t) => CellInput::Text(t.strip_prefix('\'').unwrap_or(t).to_owned()),
            Self::Null => CellInput::Clear,
        }
    }
}

fn position(p: PositionArg) -> TextPosition {
    match p {
        PositionArg::Start => TextPosition::Start,
        PositionArg::End => TextPosition::End,
    }
}

/// Zapytanie portu dla argumentów `office_read` i aplikacji pliku.
pub fn to_query(
    a: &ReadArgs,
    app: OfficeApp,
    max_chars: u32,
    table_cells: usize,
) -> Result<OfficeQuery, String> {
    let what = a.what.unwrap_or_default();
    let max_chars = a.max_chars.unwrap_or(max_chars).clamp(1, 1_000_000) as usize;
    match (what, app) {
        (ReadWhat::Text, _) => Ok(OfficeQuery::Text { max_chars }),
        (ReadWhat::Tables, OfficeApp::Word) => Ok(OfficeQuery::Tables {
            max_cells: table_cells,
        }),
        (ReadWhat::Range, OfficeApp::Excel) => {
            let range = a
                .range
                .clone()
                .ok_or("`what = range` wymaga `range` (np. `A1:D20`)")?;
            parse_range(&range).ok_or_else(|| format!("niepoprawny zakres `{range}`"))?;
            Ok(OfficeQuery::Range {
                sheet: a.sheet.clone(),
                range,
                formulas: a.formulas.unwrap_or(false),
            })
        }
        (ReadWhat::Sheets, OfficeApp::Excel) => Ok(OfficeQuery::Sheets),
        (w, app) => Err(format!(
            "`what = {}` nie dotyczy {}",
            serde_json::to_value(w)
                .ok()
                .and_then(|v| v.as_str().map(str::to_owned))
                .unwrap_or_default(),
            match app {
                OfficeApp::Word => "dokumentu Word",
                OfficeApp::Excel => "skoroszytu Excel",
            }
        )),
    }
}

/// Edycje portu z argumentów `office_edit` (walidacja limitów i formuł jak w porcie).
pub fn to_edits(a: &EditArgs) -> Result<Vec<OfficeEdit>, String> {
    if a.edits.is_empty() || a.edits.len() > platform_apps_contract::MAX_EDITS {
        return Err("lista edycji musi mieć 1–200 pozycji".into());
    }
    a.edits
        .iter()
        .map(|e| {
            let edit = match e {
                EditOp::InsertText { position: p, text } => OfficeEdit::InsertText {
                    position: position(*p),
                    text: text.clone(),
                },
                EditOp::ReplaceText { find, replace, all } => OfficeEdit::ReplaceText {
                    find: find.clone(),
                    replace: replace.clone(),
                    all: all.unwrap_or(true),
                },
                EditOp::InsertTable { position: p, rows } => OfficeEdit::InsertTable {
                    position: position(*p),
                    rows: rows.clone(),
                },
                EditOp::SetCells { sheet, start, rows } => OfficeEdit::SetCells {
                    sheet: sheet.clone(),
                    start: start.clone(),
                    rows: rows
                        .iter()
                        .map(|r| r.iter().map(CellArg::input).collect())
                        .collect(),
                },
                EditOp::AddSheet { name } => OfficeEdit::AddSheet { name: name.clone() },
            };
            edit.validate().map(|()| edit).map_err(|e| e.to_string())
        })
        .collect()
}

fn split_name(path: &str) -> (&str, &str, &str) {
    let cut = path.rfind(['\\', '/']).map_or(0, |i| i + 1);
    let (dir, name) = path.split_at(cut);
    match name.rfind('.') {
        Some(dot) if dot > 0 => (dir, &name[..dot], &name[dot..]),
        _ => (dir, name, ""),
    }
}

/// Ścieżka nowej wersji: `<katalog><nazwa> (Alfa).<ext>`, potem `(Alfa 2)`…`(Alfa 99)` — pierwsza
/// nieistniejąca; `None`, gdy wszystkie zajęte.
pub fn version_path(original: &str, exists: impl Fn(&str) -> bool) -> Option<String> {
    let (dir, stem, ext) = split_name(original);
    (1..=99)
        .map(|n| {
            if n == 1 {
                format!("{dir}{stem} (Alfa){ext}")
            } else {
                format!("{dir}{stem} (Alfa {n}){ext}")
            }
        })
        .find(|p| !exists(p))
}

/// Czy nowa wersja ma format tej samej aplikacji co oryginał.
pub fn same_app(original: &str, output: &str) -> bool {
    matches!((app_for_file(original), app_for_file(output)), (Some(a), Some(b)) if a == b)
}

/// Sprawdza argumenty (ten sam parser i reguły co implementacja).
pub fn check_args(tool: &str, args: &serde_json::Value) -> Result<(), String> {
    let v = args.clone();
    match tool {
        "office_read" => {
            let a: ReadArgs = serde_json::from_value(v).map_err(|e| e.to_string())?;
            let app = app_for_file(&a.path).ok_or("nieobsługiwany format pliku")?;
            to_query(&a, app, 1_000, 1_000).map(|_| ())
        }
        "office_edit" => {
            let a: EditArgs = serde_json::from_value(v).map_err(|e| e.to_string())?;
            let app = app_for_file(&a.path).ok_or("nieobsługiwany format pliku")?;
            let edits = to_edits(&a)?;
            if edits.iter().any(|e| e.app() != app) {
                return Err("edycja nie pasuje do rodzaju dokumentu".into());
            }
            Ok(())
        }
        other => Err(format!("nieznane narzędzie {other}")),
    }
}

/// Przykładowe poprawne argumenty.
pub fn sample_args(tool: &str) -> serde_json::Value {
    match tool {
        "office_edit" => serde_json::json!({
            "path": "/Raport.docx",
            "edits": [{"op": "insert_text", "position": "end", "text": "Podsumowanie"}]
        }),
        _ => serde_json::json!({"path": "/Raport.docx"}),
    }
}
