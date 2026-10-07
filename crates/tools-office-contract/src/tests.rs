//! Testy kontraktu `tools-office`: manifesty, argumenty, zapytania, edycje, ścieżka wersji.

use platform_apps_contract::{CellInput, OfficeApp, OfficeEdit, OfficeQuery};
use serde_json::json;

use super::*;

#[test]
fn manifests_are_valid_and_scoped() {
    let ms = manifests();
    assert_eq!(ms.len(), 2);
    for m in &ms {
        m.validate().unwrap();
        assert!(m.capabilities.contains(&"gui.control".to_owned()));
    }
    assert_eq!(ms[0].untrusted_output, Some(TaintSource::File));
    assert!(!ms[0].mutating && ms[1].mutating);
    assert!(ms[1].capabilities.contains(&"fs.write".to_owned()));
    assert!(ms[1].allowed_for(&["office".into()], false));
    assert!(
        !ms[1].allowed_for(&["office".into()], true),
        "rola tylko do odczytu"
    );
    assert_eq!(ms[0].id, "tools-office.read");
}

#[test]
fn read_queries() {
    let a = |v: serde_json::Value| serde_json::from_value::<ReadArgs>(v).unwrap();
    assert_eq!(
        to_query(&a(json!({"path": "a.docx"})), OfficeApp::Word, 100, 50).unwrap(),
        OfficeQuery::Text { max_chars: 100 }
    );
    assert_eq!(
        to_query(
            &a(json!({"path": "a.docx", "what": "tables"})),
            OfficeApp::Word,
            100,
            50
        )
        .unwrap(),
        OfficeQuery::Tables { max_cells: 50 }
    );
    let range = a(
        json!({"path": "a.xlsx", "what": "range", "range": "B2:A1", "sheet": "Dane", "formulas": true}),
    );
    assert!(matches!(
        to_query(&range, OfficeApp::Excel, 1, 1).unwrap(),
        OfficeQuery::Range { formulas: true, .. }
    ));
    assert!(
        to_query(
            &a(json!({"path": "a.xlsx", "what": "range"})),
            OfficeApp::Excel,
            1,
            1
        )
        .is_err()
    );
    assert!(
        to_query(
            &a(json!({"path": "a.docx", "what": "sheets"})),
            OfficeApp::Word,
            1,
            1
        )
        .is_err()
    );
    assert!(
        to_query(
            &a(json!({"path": "a.xlsx", "what": "range", "range": "ZZZZ1"})),
            OfficeApp::Excel,
            1,
            1
        )
        .is_err()
    );
}

#[test]
fn edits_convert_and_validate() {
    let a: EditArgs = serde_json::from_value(json!({
        "path": "a.xlsx",
        "edits": [
            {"op": "set_cells", "start": "A1", "rows": [[1, true, "=SUM(A1:A2)", "'=tekst", "x", null]]},
            {"op": "add_sheet", "name": "Nowy"}
        ]
    }))
    .unwrap();
    let edits = to_edits(&a).unwrap();
    let OfficeEdit::SetCells { rows, .. } = &edits[0] else {
        panic!("set_cells");
    };
    assert_eq!(
        rows[0],
        vec![
            CellInput::Number(1.0),
            CellInput::Bool(true),
            CellInput::Formula("=SUM(A1:A2)".into()),
            CellInput::Text("=tekst".into()),
            CellInput::Text("x".into()),
            CellInput::Clear,
        ]
    );
    let bad = |v: serde_json::Value| check_args("office_edit", &v).is_err();
    assert!(bad(
        json!({"path": "a.xlsx", "edits": [{"op": "set_cells", "start": "A1", "rows": [["=WEBSERVICE(\"http://x\")"]]}]})
    ));
    assert!(bad(
        json!({"path": "a.xlsx", "edits": [{"op": "insert_text", "position": "end", "text": "x"}]})
    ));
    assert!(bad(json!({"path": "a.docx", "edits": []})));
    assert!(bad(
        json!({"path": "a.exe", "edits": [{"op": "add_sheet", "name": "x"}]})
    ));
    assert!(bad(
        json!({"path": "a.docx", "edits": [{"op": "insert_text", "position": "end", "text": "x", "macro": "AutoOpen"}]})
    ));
    assert!(check_args("office_edit", &sample_args("office_edit")).is_ok());
    assert!(check_args("office_read", &sample_args("office_read")).is_ok());
    assert!(check_args("nic", &json!({})).is_err());
    let replace: EditArgs = serde_json::from_value(json!({"path": "a.docx", "edits": [
        {"op": "replace_text", "find": "a", "replace": "b"}]}))
    .unwrap();
    assert!(matches!(
        to_edits(&replace).unwrap()[0],
        OfficeEdit::ReplaceText { all: true, .. }
    ));
}

#[test]
fn version_paths_never_hit_original() {
    let exists = |p: &str| p.ends_with("Raport (Alfa).docx");
    assert_eq!(
        version_path(r"C:\Users\ala\Raport.docx", exists).unwrap(),
        r"C:\Users\ala\Raport (Alfa 2).docx"
    );
    assert_eq!(
        version_path("/d/notatki", |_| false).unwrap(),
        "/d/notatki (Alfa)"
    );
    assert_eq!(
        version_path("/d/.x.docx", |_| false).unwrap(),
        "/d/.x (Alfa).docx"
    );
    assert_eq!(version_path("/d/a.docx", |_| true), None);
    assert!(
        same_app("a.docx", "b.rtf")
            && !same_app("a.docx", "b.xlsx")
            && !same_app("a.docx", "b.exe")
    );
}
