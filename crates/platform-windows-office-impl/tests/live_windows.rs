//! Testy na żywo (self-hosted Windows z Office i Edge) — `#[ignore]`, uruchamiane ręcznie:
//! `cargo test -p platform-windows-office-impl --test live_windows -- --ignored`.

#![allow(clippy::unwrap_used, clippy::expect_used)]

#[cfg(windows)]
mod live {
    use std::path::PathBuf;
    use std::sync::Arc;

    use platform_apps_contract::{
        AUTOMATION_SECURITY_FORCE_DISABLE, BrowserKind, BrowserPort, BrowserSpec, EgressFilter,
        FileZone, OfficeApp, OfficeEdit, OfficeFile, OfficePort, OfficeQuery, RegKey, RegistryPort,
        TextPosition,
    };
    use platform_windows_office_impl::{
        BrowserConfig, CdpBrowser, OfficeConfig, WinOffice, WinRegistry,
    };

    fn work() -> PathBuf {
        std::env::temp_dir().join("alfa-office-live")
    }

    #[test]
    #[ignore = "wymaga Windows z zainstalowanym Wordem i pliku ALFA_LIVE_DOCX"]
    fn word_reads_and_edits_copy_with_macros_disabled() {
        let office = WinOffice::new(OfficeConfig::new(work()));
        assert!(office.available(OfficeApp::Word));
        let path = PathBuf::from(std::env::var("ALFA_LIVE_DOCX").unwrap());
        let bytes = std::fs::read(&path).unwrap();
        let file =
            OfficeFile::new(&path.to_string_lossy(), bytes.clone(), FileZone::Local).unwrap();
        let text = office
            .read(&file, &OfficeQuery::Text { max_chars: 10_000 })
            .unwrap();
        assert_eq!(
            text.session.automation_security,
            AUTOMATION_SECURITY_FORCE_DISABLE
        );
        let edits = [OfficeEdit::InsertText {
            position: TextPosition::End,
            text: "Alfa — test na żywo".into(),
        }];
        let out = office.edit(&file, &edits).unwrap();
        assert_ne!(out.bytes, bytes);
        assert_eq!(std::fs::read(&path).unwrap(), bytes, "oryginał nietknięty");
    }

    #[test]
    #[ignore = "wymaga Windows (rejestr systemu)"]
    fn registry_reads_and_denies_secrets() {
        let reg = WinRegistry::new();
        let key = RegKey::parse(r"HKLM\SOFTWARE\Microsoft\Windows NT\CurrentVersion").unwrap();
        assert!(reg.read_value(&key, "ProductName").is_ok());
        let lsa = RegKey::parse(r"HKLM\SYSTEM\CurrentControlSet\Control\Lsa").unwrap();
        assert!(reg.list(&lsa, 10).is_err());
    }

    struct Only;
    impl EgressFilter for Only {
        fn allows(&self, host: &str) -> bool {
            host == "example.com"
        }
    }

    #[test]
    #[ignore = "wymaga Windows z Microsoft Edge i dostępu do example.com"]
    fn edge_over_pipe_with_egress_filter() {
        let root = std::env::temp_dir().join("alfa-browser-live");
        let spec = BrowserSpec {
            kind: BrowserKind::Edge,
            executable: None,
            alfa_root: root.clone(),
            profile_dir: root.join("profile"),
            quarantine_dir: root.join("quarantine"),
            headless: true,
        };
        let b = CdpBrowser::system(BrowserConfig::default());
        let id = b.open(&spec, Arc::new(Only)).unwrap();
        let page = b.navigate(id, "https://example.com/").unwrap();
        assert!(page.title.contains("Example"));
        let snap = b.snapshot(id, 100, 2_000).unwrap();
        assert!(!snap.nodes.is_empty());
        assert!(
            b.navigate(id, "https://www.iana.org/").is_err()
                || !b.snapshot(id, 1, 1).unwrap().page.blocked_hosts.is_empty()
        );
        b.close(id).unwrap();
    }
}
