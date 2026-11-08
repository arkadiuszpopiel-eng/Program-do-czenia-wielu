//! Atrapa `OfficePort`: dokument w pamięci ([`FakeDocument`]), te same walidacje co
//! implementacja (`check_edits`, formuły, Protected View), dziennik sesji z `AutomationSecurity`
//! i licznik „uruchomionych” makr — uruchamiają się wyłącznie, gdy makra nie są wyłączone.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};

use platform_apps_contract::{
    AUTOMATION_SECURITY_FORCE_DISABLE, CellInput, CellValue, FileZone, MAX_RANGE_CELLS, OfficeApp,
    OfficeContent, OfficeEdit, OfficeEdited, OfficeError, OfficeFile, OfficePort, OfficeQuery,
    OfficeSession, SheetInfo, TextPosition, check_edits, parse_cell, parse_range, safe_cell_text,
};

use crate::doc::{Block, Cell, FakeDocument, Sheet};

#[derive(Debug)]
struct State {
    zones: BTreeMap<PathBuf, FileZone>,
    automation_security: i32,
    unavailable: Vec<OfficeApp>,
    sessions: Vec<OfficeSession>,
    macros_run: u32,
    opened: Vec<String>,
}

/// Atrapa Office.
#[derive(Debug)]
pub struct FakeOffice {
    state: Mutex<State>,
}

impl Default for FakeOffice {
    fn default() -> Self {
        Self {
            state: Mutex::new(State {
                zones: BTreeMap::new(),
                automation_security: AUTOMATION_SECURITY_FORCE_DISABLE,
                unavailable: Vec::new(),
                sessions: Vec::new(),
                macros_run: 0,
                opened: Vec::new(),
            }),
        }
    }
}

fn doc_err(m: &str) -> OfficeError {
    OfficeError::Document(m.to_owned())
}

impl FakeOffice {
    /// Nowa atrapa (makra wyłączone jak w implementacji).
    pub fn new() -> Self {
        Self::default()
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|p| p.into_inner())
    }

    /// Znacznik Mark-of-the-Web dla ścieżki.
    pub fn set_zone(&self, path: &Path, zone: FileZone) {
        self.lock().zones.insert(path.to_path_buf(), zone);
    }

    /// Symulacja wadliwej implementacji (test fail-closed narzędzi): inny `AutomationSecurity`.
    pub fn set_automation_security(&self, value: i32) {
        self.lock().automation_security = value;
    }

    /// Aplikacja „niezainstalowana”.
    pub fn set_unavailable(&self, app: OfficeApp) {
        self.lock().unavailable.push(app);
    }

    /// Sesje automatyzacji (dowód ustawienia `AutomationSecurity`).
    pub fn sessions(&self) -> Vec<OfficeSession> {
        self.lock().sessions.clone()
    }

    /// Ile makr „uruchomiono” (0, gdy makra są wyłączone).
    pub fn macros_run(&self) -> u32 {
        self.lock().macros_run
    }

    /// Nazwy otwartych kopii roboczych.
    pub fn opened(&self) -> Vec<String> {
        self.lock().opened.clone()
    }

    fn open(&self, file: &OfficeFile) -> Result<(FakeDocument, OfficeSession), OfficeError> {
        let mut s = self.lock();
        if s.unavailable.contains(&file.app) {
            return Err(OfficeError::NotInstalled(file.app.prog_id().into()));
        }
        let doc = FakeDocument::parse(&file.bytes)
            .ok_or_else(|| doc_err("plik uszkodzony albo chroniony hasłem"))?;
        let kind_ok = match file.app {
            OfficeApp::Word => doc.sheets.is_empty(),
            OfficeApp::Excel => doc.blocks.is_empty() && !doc.sheets.is_empty(),
        };
        if !kind_ok {
            return Err(doc_err("zawartość nie pasuje do formatu pliku"));
        }
        if s.automation_security != AUTOMATION_SECURITY_FORCE_DISABLE {
            s.macros_run += u32::try_from(doc.macros.len()).unwrap_or(u32::MAX);
        }
        let session = OfficeSession {
            automation_security: s.automation_security,
            protected_view: file.zone.is_untrusted(),
            macros_present: !doc.macros.is_empty(),
            shared_instance: false,
        };
        s.sessions.push(session);
        s.opened.push(file.file_name.clone());
        Ok((doc, session))
    }
}

fn cut(text: String, max: usize) -> (String, bool) {
    if text.chars().count() > max {
        (text.chars().take(max).collect(), true)
    } else {
        (text, false)
    }
}

fn excel_text(doc: &FakeDocument) -> String {
    let mut out = Vec::new();
    for sheet in &doc.sheets {
        out.push(format!("[{}]", sheet.name));
        let Some(((c1, r1), (c2, r2))) = parse_range(&sheet.used_range()) else {
            continue;
        };
        for r in r1..=r2 {
            let row: Vec<String> = (c1..=c2)
                .map(|c| match sheet.cells.get(&(c, r)) {
                    Some(Cell::Value(CellValue::Text(t))) => t.clone(),
                    Some(Cell::Value(CellValue::Number(n))) => n.to_string(),
                    Some(Cell::Value(CellValue::Bool(b))) => b.to_string(),
                    Some(Cell::Value(CellValue::Error { error })) => error.clone(),
                    Some(Cell::Formula(f)) => f.clone(),
                    Some(Cell::Value(CellValue::Empty)) | None => String::new(),
                })
                .collect();
            out.push(row.join("\t"));
        }
    }
    out.join("\n")
}

fn read_range(
    sheet: &Sheet,
    range: &str,
    formulas: bool,
) -> Result<Vec<Vec<CellValue>>, OfficeError> {
    let ((c1, r1), (c2, r2)) =
        parse_range(range).ok_or_else(|| doc_err(&format!("niepoprawny zakres `{range}`")))?;
    let count = u64::from(c2 - c1 + 1) * u64::from(r2 - r1 + 1);
    if count > MAX_RANGE_CELLS as u64 {
        return Err(OfficeError::Policy(
            "zakres większy niż 10 000 komórek".into(),
        ));
    }
    Ok((r1..=r2)
        .map(|r| {
            (c1..=c2)
                .map(|c| match sheet.cells.get(&(c, r)) {
                    Some(Cell::Value(v)) => v.clone(),
                    Some(Cell::Formula(f)) if formulas => CellValue::Text(f.clone()),
                    Some(Cell::Formula(_)) | None => CellValue::Empty,
                })
                .collect()
        })
        .collect())
}

fn apply(doc: &mut FakeDocument, edit: &OfficeEdit) -> Result<u32, OfficeError> {
    let insert = |doc: &mut FakeDocument, pos: TextPosition, block: Block| match pos {
        TextPosition::Start => doc.blocks.insert(0, block),
        TextPosition::End => doc.blocks.push(block),
    };
    match edit {
        OfficeEdit::InsertText { position, text } => {
            insert(doc, *position, Block::Para(text.clone()));
        }
        OfficeEdit::InsertTable { position, rows } => {
            insert(doc, *position, Block::Table(rows.clone()));
        }
        OfficeEdit::ReplaceText { find, replace, all } => {
            let mut n = 0u32;
            for b in &mut doc.blocks {
                if let Block::Para(p) = b
                    && p.contains(find.as_str())
                    && (*all || n == 0)
                {
                    let hits = u32::try_from(p.matches(find.as_str()).count()).unwrap_or(0);
                    *p = if *all {
                        n += hits;
                        p.replace(find.as_str(), replace)
                    } else {
                        n += 1;
                        p.replacen(find.as_str(), replace, 1)
                    };
                }
            }
            return Ok(n);
        }
        OfficeEdit::SetCells { sheet, start, rows } => {
            let (c0, r0) = parse_cell(start).ok_or_else(|| doc_err("niepoprawna komórka"))?;
            let target = doc
                .sheet_mut(sheet.as_deref())
                .ok_or_else(|| doc_err("brak arkusza"))?;
            for (dr, row) in (0u32..).zip(rows) {
                for (dc, input) in (0u32..).zip(row) {
                    let at = (c0 + dc, r0 + dr);
                    let cell = match input {
                        CellInput::Clear => {
                            target.cells.remove(&at);
                            continue;
                        }
                        CellInput::Text(t) => Cell::Value(CellValue::Text(safe_cell_text(t))),
                        CellInput::Number(n) => Cell::Value(CellValue::Number(*n)),
                        CellInput::Bool(b) => Cell::Value(CellValue::Bool(*b)),
                        CellInput::Formula(f) => Cell::Formula(f.clone()),
                    };
                    target.cells.insert(at, cell);
                }
            }
        }
        OfficeEdit::AddSheet { name } => {
            if doc.sheet(Some(name)).is_some() {
                return Err(doc_err(&format!("arkusz `{name}` już istnieje")));
            }
            doc.sheets.push(Sheet {
                name: name.clone(),
                cells: BTreeMap::new(),
            });
        }
    }
    Ok(0)
}

impl OfficePort for FakeOffice {
    fn available(&self, app: OfficeApp) -> bool {
        !self.lock().unavailable.contains(&app)
    }

    fn zone_of(&self, path: &Path) -> FileZone {
        self.lock().zones.get(path).copied().unwrap_or_default()
    }

    fn read(&self, file: &OfficeFile, query: &OfficeQuery) -> Result<OfficeContent, OfficeError> {
        let (doc, session) = self.open(file)?;
        let mut out = OfficeContent {
            text: None,
            tables: Vec::new(),
            cells: Vec::new(),
            sheets: Vec::new(),
            truncated: false,
            session,
        };
        match (query, file.app) {
            (OfficeQuery::Text { max_chars }, app) => {
                let text = match app {
                    OfficeApp::Word => doc.word_text(),
                    OfficeApp::Excel => excel_text(&doc),
                };
                let (text, cut) = cut(text, *max_chars);
                out.text = Some(text);
                out.truncated = cut;
            }
            (OfficeQuery::Tables { max_cells }, OfficeApp::Word) => {
                let mut left = *max_cells;
                for t in doc.tables() {
                    let n: usize = t.iter().map(Vec::len).sum();
                    if n > left {
                        out.truncated = true;
                        break;
                    }
                    left -= n;
                    out.tables.push(t);
                }
            }
            (
                OfficeQuery::Range {
                    sheet,
                    range,
                    formulas,
                },
                OfficeApp::Excel,
            ) => {
                let s = doc
                    .sheet(sheet.as_deref())
                    .ok_or_else(|| doc_err("brak arkusza"))?;
                out.cells = read_range(s, range, *formulas)?;
            }
            (OfficeQuery::Sheets, OfficeApp::Excel) => {
                out.sheets = doc
                    .sheets
                    .iter()
                    .map(|s| SheetInfo {
                        name: s.name.clone(),
                        used_range: s.used_range(),
                    })
                    .collect();
            }
            (q, app) => {
                return Err(OfficeError::Unsupported(format!(
                    "zapytanie {q:?} dla {app:?}"
                )));
            }
        }
        Ok(out)
    }

    fn edit(&self, file: &OfficeFile, edits: &[OfficeEdit]) -> Result<OfficeEdited, OfficeError> {
        check_edits(file, edits)?;
        let (mut doc, session) = self.open(file)?;
        let mut replacements = 0;
        for e in edits {
            replacements += apply(&mut doc, e)?;
        }
        Ok(OfficeEdited {
            bytes: doc.to_bytes(),
            applied: u32::try_from(edits.len()).unwrap_or(u32::MAX),
            replacements,
            session,
        })
    }
}
