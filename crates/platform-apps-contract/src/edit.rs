//! Edycje kopii roboczej dokumentu Office (Word: tekst, zamiana, tabela; Excel: komórki, formuły
//! z listy dozwolonej, arkusze) z walidacją wspólną dla portu i narzędzi.

use serde::{Deserialize, Serialize};

use crate::cells::{check_formula, check_sheet_name, parse_cell};
use crate::office::{
    MAX_EDIT_CHARS, MAX_EDITS, MAX_RANGE_CELLS, OfficeApp, OfficeError, OfficeFile, OfficeSession,
};

/// Miejsce wstawienia w dokumencie Word.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TextPosition {
    /// Na początku.
    Start,
    /// Na końcu.
    End,
}

/// Wpis komórki Excela.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum CellInput {
    /// Tekst (zawsze jako tekst — [`crate::safe_cell_text`]).
    Text(String),
    /// Liczba.
    Number(f64),
    /// Prawda/fałsz.
    Bool(bool),
    /// Formuła z listy dozwolonej (`=SUM(A1:A3)`).
    Formula(String),
    /// Wyczyszczenie komórki.
    Clear,
}

/// Edycja kopii roboczej.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum OfficeEdit {
    /// Word: wstawienie akapitu tekstu.
    InsertText {
        /// Miejsce.
        position: TextPosition,
        /// Tekst.
        text: String,
    },
    /// Word: zamiana tekstu.
    ReplaceText {
        /// Szukany tekst.
        find: String,
        /// Zamiennik.
        replace: String,
        /// Wszystkie wystąpienia (inaczej pierwsze).
        all: bool,
    },
    /// Word: wstawienie tabeli (pierwszy wiersz = nagłówek).
    InsertTable {
        /// Miejsce.
        position: TextPosition,
        /// Wiersze.
        rows: Vec<Vec<String>>,
    },
    /// Excel: wpisanie komórek od `start` (wiersze × kolumny).
    SetCells {
        /// Arkusz (`None` = pierwszy).
        sheet: Option<String>,
        /// Lewy górny róg (A1).
        start: String,
        /// Wartości.
        rows: Vec<Vec<CellInput>>,
    },
    /// Excel: nowy arkusz na końcu.
    AddSheet {
        /// Nazwa.
        name: String,
    },
}

fn too_long(text: &str) -> bool {
    text.chars().count() > MAX_EDIT_CHARS
}

impl OfficeEdit {
    /// Aplikacja, której dotyczy edycja.
    pub fn app(&self) -> OfficeApp {
        match self {
            Self::InsertText { .. } | Self::ReplaceText { .. } | Self::InsertTable { .. } => {
                OfficeApp::Word
            }
            Self::SetCells { .. } | Self::AddSheet { .. } => OfficeApp::Excel,
        }
    }

    /// Walidacja limitów, adresów, nazw arkuszy i formuł (ta sama w porcie i narzędziu).
    pub fn validate(&self) -> Result<(), OfficeError> {
        let policy = |m: &str| Err(OfficeError::Policy(m.to_owned()));
        match self {
            Self::InsertText { text, .. } if too_long(text) => policy("tekst za długi"),
            Self::ReplaceText { find, replace, .. } => {
                if find.is_empty() || find.chars().count() > 255 || replace.chars().count() > 255 {
                    return policy("szukany tekst i zamiennik: 1–255 znaków");
                }
                Ok(())
            }
            Self::InsertTable { rows, .. } => {
                let cols = rows.first().map_or(0, Vec::len);
                let cells: usize = rows.iter().map(Vec::len).sum();
                if rows.is_empty() || cols == 0 || cols > 63 || rows.len() > 1_000 {
                    return policy("tabela: 1–1000 wierszy, 1–63 kolumny");
                }
                if cells > MAX_RANGE_CELLS || rows.iter().flatten().any(|c| too_long(c)) {
                    return policy("tabela za duża");
                }
                Ok(())
            }
            Self::SetCells { sheet, start, rows } => {
                if let Some(s) = sheet {
                    check_sheet_name(s)?;
                }
                let (col, row) = parse_cell(start)
                    .ok_or_else(|| OfficeError::Policy(format!("niepoprawna komórka `{start}`")))?;
                let width = rows.iter().map(Vec::len).max().unwrap_or(0);
                let cells: usize = rows.iter().map(Vec::len).sum();
                if rows.is_empty() || width == 0 || cells > MAX_RANGE_CELLS {
                    return policy("zakres pusty albo większy niż 10 000 komórek");
                }
                let last_col = u64::from(col) + width as u64 - 1;
                let last_row = u64::from(row) + rows.len() as u64 - 1;
                if last_col > 16_384 || last_row > 1_048_576 {
                    return policy("zakres poza arkuszem");
                }
                for cell in rows.iter().flatten() {
                    match cell {
                        CellInput::Formula(f) => check_formula(f).map_err(OfficeError::Policy)?,
                        CellInput::Text(t) if too_long(t) => return policy("tekst za długi"),
                        CellInput::Number(n) if !n.is_finite() => {
                            return policy("liczba musi być skończona");
                        }
                        _ => {}
                    }
                }
                Ok(())
            }
            Self::AddSheet { name } => check_sheet_name(name),
            Self::InsertText { .. } => Ok(()),
        }
    }
}

/// Wynik edycji kopii roboczej.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OfficeEdited {
    /// Zawartość nowej wersji (ten sam format co oryginał).
    pub bytes: Vec<u8>,
    /// Ile edycji zastosowano.
    pub applied: u32,
    /// Ile zamian tekstu wykonano (suma).
    pub replacements: u32,
    /// Stan sesji.
    pub session: OfficeSession,
}

/// Wspólna walidacja wejścia edycji (port i narzędzie): strefa, liczba edycji, aplikacja, formuły.
pub fn check_edits(file: &OfficeFile, edits: &[OfficeEdit]) -> Result<(), OfficeError> {
    if file.zone.is_untrusted() {
        return Err(OfficeError::ProtectedView);
    }
    if edits.is_empty() || edits.len() > MAX_EDITS {
        return Err(OfficeError::Policy(format!("liczba edycji 1–{MAX_EDITS}")));
    }
    for e in edits {
        if e.app() != file.app {
            return Err(OfficeError::Unsupported(format!(
                "edycja dla {:?} w pliku {:?}",
                e.app(),
                file.app
            )));
        }
        e.validate()?;
    }
    Ok(())
}
