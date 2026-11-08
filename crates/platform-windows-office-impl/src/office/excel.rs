//! Excel przez `IDispatch`. Przed otwarciem kopii: pusty skoroszyt → `Calculation = ręczne`
//! (otwarty plik dziedziczy tryb — formuły sieciowe w pliku nie są liczone), bez zdarzeń, okien
//! i aktualizacji łączy (`UpdateLinks = 0`). Instancja użytkownika (widoczna/z otwartymi
//! skoroszytami) jest odrzucana — tryb obliczeń jest globalny dla instancji.
//! Odczyt: `Value2`/`Formula` jako `SAFEARRAY`; zapis: komórka po komórce (`Value2`, tekst
//! zawsze jako tekst, formuły z listy dozwolonej + `Calculate` tylko tej komórki).

use std::path::Path;

use platform_apps_contract::{
    CellInput, FileZone, MAX_RANGE_CELLS, OfficeApp, OfficeContent, OfficeEdit, OfficeError,
    OfficeQuery, OfficeSession, SheetInfo, check_formula, parse_cell, parse_range, safe_cell_text,
};
use windows::Win32::System::Variant::VARIANT;

use super::disp::{Disp, OfficeApplication, cell_grid, missing, text};
use super::sta::StaCtx;

/// Hasło-atrapa (plik chroniony → błąd zamiast okna dialogowego).
const DUMMY_PASSWORD: &str = "\u{1}alfa-brak-hasla";
/// `xlCalculationManual`.
const XL_CALCULATION_MANUAL: i32 = -4135;

struct Opened {
    office: OfficeApplication,
    wb: Disp,
    pv_window: Option<Disp>,
}

impl Opened {
    fn close(self) {
        match &self.pv_window {
            Some(w) => {
                let _ = w.call("Close", Vec::new());
            }
            None => {
                let _ = self.wb.call("Close", vec![VARIANT::from(false)]);
            }
        }
        self.office.finish();
    }
}

fn prepare(app: &Disp) -> Result<(), OfficeError> {
    for (name, value) in [
        ("DisplayAlerts", false),
        ("EnableEvents", false),
        ("AskToUpdateLinks", false),
        ("ScreenUpdating", false),
    ] {
        let _ = app.put(name, VARIANT::from(value));
    }
    // Tryb ręczny wymaga otwartego skoroszytu — pusty zostaje do `Quit`.
    app.obj("Workbooks", Vec::new())?.call("Add", Vec::new())?;
    app.put("Calculation", VARIANT::from(XL_CALCULATION_MANUAL))?;
    let _ = app.put("CalculateBeforeSave", VARIANT::from(false));
    Ok(())
}

fn open(path: &Path, zone: FileZone, read_only: bool) -> Result<Opened, OfficeError> {
    let office = OfficeApplication::start(OfficeApp::Excel)?;
    if office.shared {
        office.finish();
        return Err(OfficeError::Policy(
            "Excel jest już otwarty przez użytkownika — automatyzacja tylko w osobnej instancji"
                .into(),
        ));
    }
    let file = path.to_string_lossy();
    let opened = prepare(&office.app).and_then(|()| {
        if zone.is_untrusted() {
            // `ProtectedViewWindows.Open(Filename, Password, AddToMru, RepairMode)`.
            let w = office.app.obj("ProtectedViewWindows", Vec::new())?.obj(
                "Open",
                vec![
                    text(&file),
                    text(DUMMY_PASSWORD),
                    VARIANT::from(false),
                    VARIANT::from(false),
                ],
            )?;
            let wb = w.obj("Workbook", Vec::new())?;
            Ok((wb, Some(w)))
        } else {
            // `Open(Filename, UpdateLinks, ReadOnly, Format, Password, WriteResPassword,
            //  IgnoreReadOnlyRecommended, Origin, Delimiter, Editable, Notify, Converter, AddToMru)`.
            let wb = office.app.obj("Workbooks", Vec::new())?.obj(
                "Open",
                vec![
                    text(&file),
                    VARIANT::from(0i32),
                    VARIANT::from(read_only),
                    missing(),
                    text(DUMMY_PASSWORD),
                    text(DUMMY_PASSWORD),
                    VARIANT::from(true),
                    missing(),
                    missing(),
                    VARIANT::from(false),
                    VARIANT::from(false),
                    missing(),
                    VARIANT::from(false),
                ],
            )?;
            Ok((wb, None))
        }
    });
    match opened {
        Ok((wb, pv_window)) => Ok(Opened {
            office,
            wb,
            pv_window,
        }),
        Err(e) => {
            office.finish();
            Err(e)
        }
    }
}

fn sheet(wb: &Disp, name: Option<&str>) -> Result<Disp, OfficeError> {
    let sheets = wb.obj("Worksheets", Vec::new())?;
    let key = name.map_or_else(|| VARIANT::from(1i32), text);
    sheets
        .obj("Item", vec![key])
        .map_err(|_| OfficeError::Document(format!("brak arkusza {}", name.unwrap_or("1"))))
}

fn used_range(ws: &Disp) -> Result<String, OfficeError> {
    let used = ws.obj("UsedRange", Vec::new())?;
    // `Address(RowAbsolute = False, ColumnAbsolute = False)`.
    let v = used.get_with("Address", vec![VARIANT::from(false), VARIANT::from(false)])?;
    Ok(super::disp::variant_text(&v))
}

fn has_macros(wb: &Disp) -> bool {
    wb.get("HasVBProject")
        .ok()
        .and_then(|v| bool::try_from(&v).ok())
        .unwrap_or(false)
}

fn cell_count(range: &str) -> Result<usize, OfficeError> {
    let ((c1, r1), (c2, r2)) = parse_range(range)
        .ok_or_else(|| OfficeError::Document(format!("niepoprawny zakres `{range}`")))?;
    let n = u64::from(c2 - c1 + 1) * u64::from(r2 - r1 + 1);
    usize::try_from(n).map_err(|_| OfficeError::Policy("zakres za duży".into()))
}

fn query_wb(
    wb: &Disp,
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
        OfficeQuery::Range {
            sheet: name,
            range,
            formulas,
        } => {
            if cell_count(range)? > MAX_RANGE_CELLS {
                return Err(OfficeError::Policy(
                    "zakres większy niż 10 000 komórek".into(),
                ));
            }
            let ws = sheet(wb, name.as_deref())?;
            let rng = ws.obj("Range", vec![text(range)])?;
            let prop = if *formulas { "Formula" } else { "Value2" };
            out.cells = cell_grid(&rng.get(prop)?)?;
        }
        OfficeQuery::Sheets => {
            let sheets = wb.obj("Worksheets", Vec::new())?;
            for i in 1..=sheets.int("Count")?.min(255) {
                let ws = sheets.obj("Item", vec![VARIANT::from(i)])?;
                out.sheets.push(SheetInfo {
                    name: ws.string("Name")?,
                    used_range: used_range(&ws)?,
                });
            }
        }
        OfficeQuery::Text { max_chars } => {
            let sheets = wb.obj("Worksheets", Vec::new())?;
            let (mut text_out, mut budget) = (String::new(), MAX_RANGE_CELLS);
            for i in 1..=sheets.int("Count")?.min(50) {
                let ws = sheets.obj("Item", vec![VARIANT::from(i)])?;
                let used = used_range(&ws)?;
                let n = cell_count(&used)?;
                text_out.push_str(&format!("[{}]\n", ws.string("Name")?));
                if n > budget {
                    out.truncated = true;
                    break;
                }
                budget -= n;
                let grid = cell_grid(&ws.obj("Range", vec![text(&used)])?.get("Value2")?)?;
                for row in grid {
                    let cells: Vec<String> = row
                        .iter()
                        .map(|c| {
                            serde_json::to_value(c).map_or_else(
                                |_| String::new(),
                                |v| match v {
                                    serde_json::Value::String(s) => s,
                                    serde_json::Value::Null => String::new(),
                                    other => other.to_string(),
                                },
                            )
                        })
                        .collect();
                    text_out.push_str(&cells.join("\t"));
                    text_out.push('\n');
                }
            }
            out.truncated |= text_out.chars().count() > *max_chars;
            out.text = Some(text_out.chars().take(*max_chars).collect());
        }
        OfficeQuery::Tables { .. } => {
            return Err(OfficeError::Unsupported(
                "tabele Worda w skoroszycie — użyj zakresu".into(),
            ));
        }
    }
    Ok(out)
}

/// Odczyt skoroszytu.
pub(crate) fn read(
    _ctx: &mut StaCtx,
    path: &Path,
    zone: FileZone,
    query: &OfficeQuery,
) -> Result<OfficeContent, OfficeError> {
    let opened = open(path, zone, true)?;
    let session = opened
        .office
        .session(opened.pv_window.is_some(), has_macros(&opened.wb));
    let result = query_wb(&opened.wb, query, session);
    opened.close();
    result
}

fn set_cell(cell: &Disp, input: &CellInput) -> Result<(), OfficeError> {
    match input {
        CellInput::Clear => cell.call("ClearContents", Vec::new()).map(|_| ()),
        CellInput::Text(t) => cell.put("Value2", text(&safe_cell_text(t))),
        CellInput::Number(n) => cell.put("Value2", VARIANT::from(*n)),
        CellInput::Bool(b) => cell.put("Value2", VARIANT::from(*b)),
        CellInput::Formula(f) => {
            check_formula(f).map_err(OfficeError::Policy)?;
            cell.put("Formula", text(f))?;
            cell.call("Calculate", Vec::new()).map(|_| ())
        }
    }
}

fn apply(wb: &Disp, edit: &OfficeEdit) -> Result<(), OfficeError> {
    match edit {
        OfficeEdit::SetCells {
            sheet: name,
            start,
            rows,
        } => {
            let (c0, r0) = parse_cell(start)
                .ok_or_else(|| OfficeError::Policy(format!("niepoprawna komórka `{start}`")))?;
            let ws = sheet(wb, name.as_deref())?;
            for (dr, row) in (0u32..).zip(rows) {
                for (dc, input) in (0u32..).zip(row) {
                    let (r, c) = (
                        i32::try_from(r0 + dr).unwrap_or(1),
                        i32::try_from(c0 + dc).unwrap_or(1),
                    );
                    let cell = ws.obj("Cells", vec![VARIANT::from(r), VARIANT::from(c)])?;
                    set_cell(&cell, input)?;
                }
            }
            Ok(())
        }
        OfficeEdit::AddSheet { name } => {
            let sheets = wb.obj("Worksheets", Vec::new())?;
            let last = sheets.obj("Item", vec![VARIANT::from(sheets.int("Count")?)])?;
            // `Add(Before, After = ostatni)`.
            let ws = sheets.obj("Add", vec![missing(), VARIANT::from(last.0.clone())])?;
            ws.put("Name", text(name))
        }
        other => Err(OfficeError::Unsupported(format!(
            "edycja {other:?} w skoroszycie"
        ))),
    }
}

/// Edycje kopii roboczej i zapis (format bez zmian).
pub(crate) fn edit(
    _ctx: &mut StaCtx,
    path: &Path,
    edits: &[OfficeEdit],
) -> Result<(u32, u32, OfficeSession), OfficeError> {
    let opened = open(path, FileZone::Local, false)?;
    let session = opened.office.session(false, has_macros(&opened.wb));
    let result = edits
        .iter()
        .try_for_each(|e| apply(&opened.wb, e))
        .and_then(|()| opened.wb.call("Save", Vec::new()).map(|_| ()));
    opened.close();
    result.map(|()| (u32::try_from(edits.len()).unwrap_or(u32::MAX), 0, session))
}
