//! Word przez `IDispatch`: otwarcie kopii roboczej (`Documents.Open` z hasłami-atrapami, żeby
//! dokument chroniony nie zawiesił się na oknie hasła; plik z Internetu —
//! `ProtectedViewWindows.Open`, tylko odczyt), tekst, tabele, wstawianie akapitów i tabel,
//! zamiana tekstu (bez symboli wieloznacznych), zapis kopii.

use std::path::Path;

use platform_apps_contract::{
    FileZone, OfficeApp, OfficeContent, OfficeEdit, OfficeError, OfficeQuery, OfficeSession,
    TextPosition,
};
use windows::Win32::System::Variant::VARIANT;

use super::disp::{Disp, OfficeApplication, text};
use super::sta::StaCtx;

/// Hasło-atrapa: dokument chroniony hasłem kończy się błędem zamiast okna dialogowego.
const DUMMY_PASSWORD: &str = "\u{1}alfa-brak-hasla";

/// Otwarty dokument z aplikacją (zamknięcie bez zapisu w `close`).
struct Opened {
    office: OfficeApplication,
    doc: Disp,
    pv_window: Option<Disp>,
}

impl Opened {
    fn close(self) {
        match &self.pv_window {
            Some(w) => {
                let _ = w.call("Close", Vec::new());
            }
            // `Close(SaveChanges = wdDoNotSaveChanges)`.
            None => {
                let _ = self.doc.call("Close", vec![VARIANT::from(0i32)]);
            }
        }
        self.office.finish();
    }
}

fn open(path: &Path, zone: FileZone, read_only: bool) -> Result<Opened, OfficeError> {
    let office = OfficeApplication::start(OfficeApp::Word)?;
    let app = &office.app;
    // Bez okien dialogowych i bez aktualizacji łączy przy otwarciu (pobieranie z sieci).
    let _ = app.put("DisplayAlerts", VARIANT::from(0i32));
    if let Ok(options) = app.obj("Options", Vec::new()) {
        let _ = options.put("UpdateLinksAtOpen", VARIANT::from(false));
        let _ = options.put("ConfirmConversions", VARIANT::from(false));
    }
    let file = path.to_string_lossy();
    let opened = if zone.is_untrusted() {
        let windows = app.obj("ProtectedViewWindows", Vec::new())?;
        // `Open(FileName, AddToRecentFiles, PasswordDocument, Visible)`.
        let pv = windows.obj(
            "Open",
            vec![
                text(&file),
                VARIANT::from(false),
                text(DUMMY_PASSWORD),
                VARIANT::from(false),
            ],
        );
        match pv.and_then(|w| w.obj("Document", Vec::new()).map(|d| (w, d))) {
            Ok((w, doc)) => Ok((doc, Some(w))),
            Err(e) => Err(e),
        }
    } else {
        // `Open(FileName, ConfirmConversions, ReadOnly, AddToRecentFiles, PasswordDocument,
        //  PasswordTemplate, Revert, WritePasswordDocument)`.
        app.obj("Documents", Vec::new())
            .and_then(|docs| {
                docs.obj(
                    "Open",
                    vec![
                        text(&file),
                        VARIANT::from(false),
                        VARIANT::from(read_only),
                        VARIANT::from(false),
                        text(DUMMY_PASSWORD),
                        text(DUMMY_PASSWORD),
                        VARIANT::from(false),
                        text(DUMMY_PASSWORD),
                    ],
                )
            })
            .map(|d| (d, None))
    };
    match opened {
        Ok((doc, pv_window)) => Ok(Opened {
            office,
            doc,
            pv_window,
        }),
        Err(e) => {
            office.finish();
            Err(e)
        }
    }
}

fn clean(cell: &str) -> String {
    cell.trim_end_matches(['\r', '\u{7}'])
        .replace(['\r', '\u{b}'], "\n")
        .replace('\u{7}', "")
}

fn has_macros(doc: &Disp) -> bool {
    doc.get("HasVBProject")
        .ok()
        .and_then(|v| bool::try_from(&v).ok())
        .unwrap_or(false)
}

/// Tabele dokumentu: wiersze × komórki.
type Tables = Vec<Vec<Vec<String>>>;

fn tables(doc: &Disp, max_cells: usize) -> Result<(Tables, bool), OfficeError> {
    let all = doc.obj("Tables", Vec::new())?;
    let count = all.int("Count")?;
    let (mut out, mut left) = (Vec::new(), max_cells);
    for i in 1..=count {
        let t = all.obj("Item", vec![VARIANT::from(i)])?;
        let rows = t.obj("Rows", Vec::new())?.int("Count")?;
        let cols = t.obj("Columns", Vec::new())?.int("Count")?;
        let need = usize::try_from(rows.max(0) * cols.max(0)).unwrap_or(usize::MAX);
        if need > left {
            return Ok((out, true));
        }
        left -= need;
        let mut grid = Vec::new();
        for r in 1..=rows {
            let row = (1..=cols)
                .map(|c| {
                    t.obj("Cell", vec![VARIANT::from(r), VARIANT::from(c)])
                        .and_then(|cell| cell.obj("Range", Vec::new()))
                        .and_then(|rng| rng.string("Text"))
                        .map(|s| clean(&s))
                        .unwrap_or_default()
                })
                .collect();
            grid.push(row);
        }
        out.push(grid);
    }
    Ok((out, false))
}

/// Odczyt dokumentu.
pub(crate) fn read(
    _ctx: &mut StaCtx,
    path: &Path,
    zone: FileZone,
    query: &OfficeQuery,
) -> Result<OfficeContent, OfficeError> {
    let opened = open(path, zone, true)?;
    let session = opened
        .office
        .session(opened.pv_window.is_some(), has_macros(&opened.doc));
    let result = query_doc(&opened.doc, query, session);
    opened.close();
    result
}

fn query_doc(
    doc: &Disp,
    query: &OfficeQuery,
    session: OfficeSession,
) -> Result<OfficeContent, OfficeError> {
    let mut out = OfficeContent {
        text: None,
        tables: Vec::new(),
        cells: Vec::new(),
        sheets: Vec::new(),
        truncated: false,
        session,
    };
    match query {
        OfficeQuery::Text { max_chars } => {
            let raw = clean(&doc.obj("Content", Vec::new())?.string("Text")?);
            out.truncated = raw.chars().count() > *max_chars;
            out.text = Some(raw.chars().take(*max_chars).collect());
        }
        OfficeQuery::Tables { max_cells } => {
            (out.tables, out.truncated) = tables(doc, *max_cells)?;
        }
        other => {
            return Err(OfficeError::Unsupported(format!(
                "zapytanie {other:?} dla dokumentu Word"
            )));
        }
    }
    Ok(out)
}

fn count(doc: &Disp, needle: &str) -> usize {
    doc.obj("Content", Vec::new())
        .and_then(|c| c.string("Text"))
        .map(|t| t.matches(needle).count())
        .unwrap_or(0)
}

fn insert_point(doc: &Disp, position: TextPosition) -> Result<Disp, OfficeError> {
    match position {
        TextPosition::Start => doc.obj("Range", vec![VARIANT::from(0i32), VARIANT::from(0i32)]),
        TextPosition::End => {
            let content = doc.obj("Content", Vec::new())?;
            content.call("InsertParagraphAfter", Vec::new())?;
            let end = doc.obj("Content", Vec::new())?.int("End")? - 1;
            doc.obj("Range", vec![VARIANT::from(end), VARIANT::from(end)])
        }
    }
}

fn apply(doc: &Disp, edit: &OfficeEdit) -> Result<u32, OfficeError> {
    match edit {
        OfficeEdit::InsertText {
            position: TextPosition::Start,
            text: t,
        } => {
            doc.obj("Content", Vec::new())?
                .call("InsertBefore", vec![text(&format!("{t}\r"))])?;
        }
        OfficeEdit::InsertText {
            position: TextPosition::End,
            text: t,
        } => {
            let content = doc.obj("Content", Vec::new())?;
            content.call("InsertParagraphAfter", Vec::new())?;
            content.call("InsertAfter", vec![text(t)])?;
        }
        OfficeEdit::ReplaceText { find, replace, all } => {
            let before = count(doc, find);
            let finder = doc.obj("Content", Vec::new())?.obj("Find", Vec::new())?;
            finder.call("ClearFormatting", Vec::new())?;
            // `Execute(FindText, MatchCase, MatchWholeWord, MatchWildcards, MatchSoundsLike,
            //  MatchAllWordForms, Forward, Wrap = wdFindStop, Format, ReplaceWith,
            //  Replace = wdReplaceAll | wdReplaceOne)`.
            finder.call(
                "Execute",
                vec![
                    text(find),
                    VARIANT::from(true),
                    VARIANT::from(false),
                    VARIANT::from(false),
                    VARIANT::from(false),
                    VARIANT::from(false),
                    VARIANT::from(true),
                    VARIANT::from(0i32),
                    VARIANT::from(false),
                    text(replace),
                    VARIANT::from(if *all { 2i32 } else { 1i32 }),
                ],
            )?;
            let after = if replace.contains(find.as_str()) {
                0
            } else {
                count(doc, find)
            };
            let n = if replace.contains(find.as_str()) {
                if *all { before } else { before.min(1) }
            } else {
                before.saturating_sub(after)
            };
            return Ok(u32::try_from(n).unwrap_or(u32::MAX));
        }
        OfficeEdit::InsertTable { position, rows } => {
            let at = insert_point(doc, *position)?;
            let cols = rows.first().map_or(0, Vec::len);
            let (nr, nc) = (
                i32::try_from(rows.len()).unwrap_or(0),
                i32::try_from(cols).unwrap_or(0),
            );
            let table = doc.obj("Tables", Vec::new())?.obj(
                "Add",
                vec![
                    VARIANT::from(at.0.clone()),
                    VARIANT::from(nr),
                    VARIANT::from(nc),
                ],
            )?;
            if let Ok(borders) = table.obj("Borders", Vec::new()) {
                let _ = borders.put("Enable", VARIANT::from(true));
            }
            for (r, row) in (1i32..).zip(rows) {
                for (c, value) in (1i32..).zip(row) {
                    table
                        .obj("Cell", vec![VARIANT::from(r), VARIANT::from(c)])?
                        .obj("Range", Vec::new())?
                        .put("Text", text(value))?;
                }
            }
        }
        other => {
            return Err(OfficeError::Unsupported(format!(
                "edycja {other:?} w dokumencie Word"
            )));
        }
    }
    Ok(0)
}

/// Edycje kopii roboczej i zapis (format bez zmian).
pub(crate) fn edit(
    _ctx: &mut StaCtx,
    path: &Path,
    edits: &[OfficeEdit],
) -> Result<(u32, u32, OfficeSession), OfficeError> {
    let opened = open(path, FileZone::Local, false)?;
    let session = opened.office.session(false, has_macros(&opened.doc));
    let result = (|| {
        let mut replacements = 0;
        for e in edits {
            replacements += apply(&opened.doc, e)?;
        }
        opened.doc.call("Save", Vec::new())?;
        Ok((u32::try_from(edits.len()).unwrap_or(u32::MAX), replacements))
    })();
    opened.close();
    result.map(|(a, r)| (a, r, session))
}
