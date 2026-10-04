//! Atrapy portów aplikacji: Office (makra wyłączone, Protected View, kopia robocza), rejestr
//! (deny-lista przed odczytem), przeglądarka (filtr egressu, kwarantanna, hasła, profil).

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::{Path, PathBuf};
use std::sync::Arc;

use platform_apps_contract::{
    AUTOMATION_SECURITY_FORCE_DISABLE, BrowserError, BrowserKind, BrowserPort, BrowserSpec,
    CellInput, CellValue, EgressFilter, FileZone, OfficeEdit, OfficeError, OfficeFile, OfficePort,
    OfficeQuery, RegData, RegKey, RegistryError, RegistryPort, TextPosition,
};
use platform_apps_fake::{FakeBrowser, FakeDocument, FakeNode, FakeOffice, FakePage, FakeRegistry};

fn word_file(doc: &FakeDocument, zone: FileZone) -> OfficeFile {
    OfficeFile::new("Raport.docx", doc.to_bytes(), zone).unwrap()
}

#[test]
fn office_edits_copy_and_never_runs_macros() {
    let office = FakeOffice::new();
    let doc = FakeDocument::word(&["Wstęp", "Kwota: 100 zł"])
        .with_table(&[&["a", "b"], &["1", "2"]])
        .with_macro("AutoOpen");
    let file = word_file(&doc, FileZone::Local);
    let text = office
        .read(&file, &OfficeQuery::Text { max_chars: 1_000 })
        .unwrap();
    assert!(text.text.unwrap().contains("Kwota"));
    assert!(text.session.macros_present);
    let tables = office
        .read(&file, &OfficeQuery::Tables { max_cells: 100 })
        .unwrap();
    assert_eq!(tables.tables[0][1], vec!["1".to_owned(), "2".to_owned()]);
    let edits = [
        OfficeEdit::ReplaceText {
            find: "100".into(),
            replace: "200".into(),
            all: true,
        },
        OfficeEdit::InsertText {
            position: TextPosition::End,
            text: "Podpis".into(),
        },
    ];
    let out = office.edit(&file, &edits).unwrap();
    assert_eq!((out.applied, out.replacements), (2, 1));
    let new = FakeDocument::parse(&out.bytes).unwrap();
    assert!(new.word_text().contains("200 zł") && new.word_text().ends_with("Podpis"));
    assert_eq!(
        FakeDocument::parse(&file.bytes).unwrap(),
        doc,
        "oryginał nietknięty"
    );
    assert_eq!(office.macros_run(), 0);
    assert!(
        office
            .sessions()
            .iter()
            .all(|s| s.automation_security == AUTOMATION_SECURITY_FORCE_DISABLE)
    );
    office.set_automation_security(1);
    let bad = office
        .read(&file, &OfficeQuery::Text { max_chars: 10 })
        .unwrap();
    assert_eq!(bad.session.automation_security, 1);
    assert_eq!(
        office.macros_run(),
        1,
        "atrapa pokazuje skutek makr przy złym ustawieniu"
    );
}

#[test]
fn office_protected_view_and_excel() {
    let office = FakeOffice::new();
    let web = word_file(&FakeDocument::word(&["z sieci"]), FileZone::Internet);
    let read = office
        .read(&web, &OfficeQuery::Text { max_chars: 100 })
        .unwrap();
    assert!(read.session.protected_view);
    let edit = [OfficeEdit::InsertText {
        position: TextPosition::Start,
        text: "x".into(),
    }];
    assert_eq!(office.edit(&web, &edit), Err(OfficeError::ProtectedView));
    office.set_zone(Path::new("/d/a.docx"), FileZone::Internet);
    assert_eq!(office.zone_of(Path::new("/d/a.docx")), FileZone::Internet);
    assert_eq!(office.zone_of(Path::new("/d/b.docx")), FileZone::Local);
    let sheet = FakeDocument::excel("Dane").with_cell("A1", CellValue::Number(2.0));
    let xls = OfficeFile::new("t.xlsx", sheet.to_bytes(), FileZone::Local).unwrap();
    let edits = [OfficeEdit::SetCells {
        sheet: None,
        start: "B1".into(),
        rows: vec![vec![
            CellInput::Formula("=A1*2".into()),
            CellInput::Text("=cmd|'/c calc'!A0".into()),
        ]],
    }];
    let out = office.edit(&xls, &edits).unwrap();
    let edited = OfficeFile::new("t.xlsx", out.bytes, FileZone::Local).unwrap();
    let cells = office
        .read(
            &edited,
            &OfficeQuery::Range {
                sheet: Some("dane".into()),
                range: "A1:C1".into(),
                formulas: true,
            },
        )
        .unwrap();
    assert_eq!(
        cells.cells[0],
        vec![
            CellValue::Number(2.0),
            CellValue::Text("=A1*2".into()),
            CellValue::Text("'=cmd|'/c calc'!A0".into())
        ]
    );
    let dde = [OfficeEdit::SetCells {
        sheet: None,
        start: "A2".into(),
        rows: vec![vec![CellInput::Formula("=cmd|'/c calc'!A0".into())]],
    }];
    assert!(matches!(
        office.edit(&xls, &dde),
        Err(OfficeError::Policy(_))
    ));
    let sheets = office.read(&edited, &OfficeQuery::Sheets).unwrap();
    assert_eq!(sheets.sheets[0].used_range, "A1:C1");
    assert!(
        office
            .read(&xls, &OfficeQuery::Tables { max_cells: 1 })
            .is_err()
    );
    let broken = OfficeFile::new("x.docx", b"PK\x03\x04".to_vec(), FileZone::Local).unwrap();
    assert!(matches!(
        office.read(&broken, &OfficeQuery::Text { max_chars: 1 }),
        Err(OfficeError::Document(_))
    ));
}

#[test]
fn registry_guard_runs_before_store() {
    let reg = FakeRegistry::new();
    reg.set(r"HKCU\Software\Acme", "Wersja", RegData::Dword(3));
    reg.set(
        r"HKCU\Software\Acme",
        "ApiToken",
        RegData::String("SEKRET-1".into()),
    );
    reg.set(
        r"HKCU\Software\Acme\Credentials",
        "x",
        RegData::String("SEKRET-2".into()),
    );
    reg.set(
        r"HKCU\Software\Acme\Ustawienia",
        "Kolor",
        RegData::String("zielony".into()),
    );
    let acme = RegKey::parse(r"hkcu/software/ACME").unwrap();
    let listing = reg.list(&acme, 100).unwrap();
    assert_eq!(listing.subkeys, vec!["Ustawienia".to_owned()]);
    assert_eq!(listing.hidden_subkeys, 1);
    assert!(!format!("{listing:?}").contains("SEKRET"));
    assert_eq!(
        reg.read_value(&acme, "apitoken").unwrap().data,
        RegData::Redacted
    );
    let creds = RegKey::parse(r"HKCU\Software\Acme\Credentials").unwrap();
    assert!(matches!(
        reg.list(&creds, 10),
        Err(RegistryError::Denied(_))
    ));
    assert!(matches!(
        reg.read_value(&creds, "x"),
        Err(RegistryError::Denied(_))
    ));
    assert_eq!(reg.raw_secret_reads(), 0);
    let missing = RegKey::parse(r"HKLM\SOFTWARE\Brak").unwrap();
    assert!(matches!(
        reg.list(&missing, 10),
        Err(RegistryError::NotFound(_))
    ));
    assert!(reg.reads() >= 3);
}

struct Hosts(Vec<&'static str>);
impl EgressFilter for Hosts {
    fn allows(&self, host: &str) -> bool {
        self.0.contains(&host)
    }
}

/// Ścieżka bezwzględna właściwa dla systemu, składana segment po segmencie (`rel` — segmenty
/// rozdzielone `/`): na Windows `/alfa` nie ma litery dysku, więc nie jest bezwzględna i
/// `BrowserSpec::validate` odrzuca ją, zanim dojdzie do profilu.
fn abs(rel: &str) -> PathBuf {
    let root = PathBuf::from(if cfg!(windows) { r"C:\" } else { "/" });
    rel.split('/')
        .filter(|s| !s.is_empty())
        .fold(root, |acc, seg| acc.join(seg))
}

fn spec() -> BrowserSpec {
    BrowserSpec {
        kind: BrowserKind::Edge,
        executable: None,
        alfa_root: abs("alfa"),
        profile_dir: abs("alfa/browser/profile"),
        quarantine_dir: abs("alfa/browser/quarantine"),
        headless: true,
    }
}

#[test]
fn browser_egress_quarantine_and_passwords() {
    let b = FakeBrowser::new();
    b.add_page(
        "https://sklep.pl/",
        FakePage {
            title: "Sklep".into(),
            text: "Oferta".into(),
            nodes: vec![
                FakeNode {
                    href: Some("https://inny.example/".into()),
                    ..FakeNode::new("link", "Inny")
                },
                FakeNode {
                    download: Some(("https://sklep.pl/f.pdf".into(), "f.pdf".into(), 10)),
                    ..FakeNode::new("link", "Faktura")
                },
                FakeNode {
                    password: true,
                    value: Some("tajne".into()),
                    ..FakeNode::new("textbox", "Hasło")
                },
            ],
            resources: vec![
                "https://cdn.tracker.net/t.js".into(),
                "data:image/png,x".into(),
            ],
        },
    );
    // Profil przeglądarki użytkownika (Linux i Windows) — odmowa, zanim cokolwiek się uruchomi.
    for user in [
        "home/u/.config/google-chrome/Default",
        "Users/u/AppData/Local/Google/Chrome/User Data/Default",
        "alfa/../Users/u/AppData/Local/Microsoft/Edge/User Data",
    ] {
        let mut bad = spec();
        bad.profile_dir = abs(user);
        assert!(
            matches!(
                b.open(&bad, Arc::new(Hosts(vec![]))),
                Err(BrowserError::Policy(_))
            ),
            "{user}"
        );
    }
    assert!(b.launches().is_empty());
    let id = b.open(&spec(), Arc::new(Hosts(vec!["sklep.pl"]))).unwrap();
    assert!(b.launches()[0].contains(&"--remote-debugging-pipe".to_owned()));
    assert!(matches!(
        b.navigate(id, "https://zly.example/"),
        Err(BrowserError::Blocked(h)) if h == "zly.example"
    ));
    let page = b.navigate(id, "https://sklep.pl/").unwrap();
    assert_eq!(page.blocked_hosts, vec!["cdn.tracker.net".to_owned()]);
    let snap = b.snapshot(id, 50, 100).unwrap();
    assert_eq!(snap.nodes[2].value, None);
    assert!(snap.nodes[2].password);
    assert_eq!(
        b.type_text(id, 3, "x", false),
        Err(BrowserError::PasswordField)
    );
    let dl = b.click(id, 2).unwrap();
    assert!(dl.downloads[0].path.starts_with(spec().quarantine_dir));
    let other = b.click(id, 1).unwrap();
    assert_eq!(other.url, "https://sklep.pl/");
    assert_eq!(other.blocked_hosts, vec!["inny.example".to_owned()]);
    for (url, allowed) in b.network() {
        let host = platform_apps_contract::url_host(&url);
        assert_eq!(
            allowed,
            host.as_deref().is_none_or(|h| h == "sklep.pl"),
            "{url}"
        );
    }
    assert!(b.screenshot(id, 100).unwrap().starts_with(b"\x89PNG"));
    b.close(id).unwrap();
    assert!(b.close(id).is_err());
    assert_eq!(b.open_sessions(), 0);
}
