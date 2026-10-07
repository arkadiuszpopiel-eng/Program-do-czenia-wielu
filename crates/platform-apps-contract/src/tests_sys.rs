//! Testy polityk portu systemu i kwarantanny pobrań: procesy chronione (drzewo Alfy, krytyczne,
//! aliasy), tożsamość procesu, usługi krytyczne, zmienne (sekrety, deny-lista zapisu), XPath bez
//! wstrzyknięcia, nazwy pobranych plików (property: zawsze bezpieczna nazwa).

use platform_contract::TargetGuard;
use proptest::prelude::*;

use crate::*;

fn entry(pid: u32, parent: u32, image: &str) -> ProcessEntry {
    ProcessEntry {
        pid,
        parent_pid: parent,
        image: image.into(),
        session_id: Some(1),
        own: Some(true),
        threads: 1,
    }
}

#[test]
fn alfa_tree_kernel_and_critical_processes_are_protected() {
    let me = std::process::id();
    let guard = TargetGuard::baseline().with_pids([900]);
    let all = vec![
        entry(me, 1, "alfa.exe"),
        entry(5000, me, "msedgewebview2.exe"),
        entry(5001, 5000, "msedgewebview2.exe"),
        entry(900, 1, "launcher.exe"),
        entry(901, 900, "conhost.exe"),
        entry(7000, 1, "notepad.exe"),
        entry(7001, 1, "ALFA-B~1.EXE"),
        entry(7002, 1, "alfa-watchdog.exe"),
        entry(7003, 1, "Explorer.EXE"),
        entry(7004, 1, ""),
        entry(4, 0, "System"),
    ];
    let protected: Vec<u32> = all
        .iter()
        .filter(|e| is_protected_entry(&guard, e, &all))
        .map(|e| e.pid)
        .collect();
    assert_eq!(
        protected,
        vec![me, 5000, 5001, 900, 901, 7001, 7002, 7003, 7004, 4]
    );
    assert!(is_critical_process(r"C:\Windows\System32\LSASS.EXE"));
    assert!(!is_critical_process("notepad.exe"));
}

#[test]
fn identity_detects_pid_reuse() {
    let id = ProcessIdentity {
        pid: 10,
        image: "notepad.exe".into(),
        started_ms: Some(1_000),
    };
    assert!(id.matches(r"C:\Windows\NOTEPAD.EXE", Some(1_000)));
    assert!(!id.matches("notepad.exe", Some(2_000)), "inny czas startu");
    assert!(!id.matches("calc.exe", Some(1_000)), "inny obraz");
    assert!(
        !id.matches("notepad.exe", None),
        "czas nieznany przy porównaniu"
    );
    let loose = ProcessIdentity {
        started_ms: None,
        ..id
    };
    assert!(loose.matches("notepad.exe", Some(5)));
}

#[test]
fn services_and_names() {
    for s in [
        "AlfaBroker",
        "WinDefend",
        "mpssvc",
        "EventLog",
        "alfa-cokolwiek",
    ] {
        assert!(is_critical_service(s), "{s}");
    }
    assert!(!is_critical_service("Spooler"));
    assert!(check_service_name("Spooler").is_ok());
    for bad in ["", "a/b", "a\\b", "x\"", "a\nb"] {
        assert!(check_service_name(bad).is_err(), "{bad:?}");
    }
    assert_eq!(ServiceState::from_win32(4), ServiceState::Running);
    assert_eq!(ServiceState::from_win32(99), ServiceState::Unknown);
}

#[test]
fn env_secrets_hidden_and_writes_denied() {
    let vars = guard_env(vec![
        ("Path".into(), r"C:\bin".into()),
        ("OPENAI_API_KEY".into(), "sk-xyz".into()),
        ("GITHUB_TOKEN".into(), "ghp_x".into()),
        ("db_password".into(), "x".into()),
    ]);
    let hidden: Vec<&str> = vars
        .iter()
        .filter(|v| v.value.is_none())
        .map(|v| v.name.as_str())
        .collect();
    assert_eq!(
        hidden,
        vec!["db_password", "GITHUB_TOKEN", "OPENAI_API_KEY"]
    );
    for denied in [
        "APPDATA",
        "localappdata",
        "USERPROFILE",
        "TEMP",
        "ComSpec",
        "PATHEXT",
        "ALFA_DATA_DIR",
        "WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS",
        "COR_PROFILER_PATH",
        "DOTNET_STARTUP_HOOKS",
        "HTTPS_PROXY",
        "no_proxy",
        "SSLKEYLOGFILE",
        "SSL_CERT_FILE",
        "NODE_OPTIONS",
        "GIT_SSH_COMMAND",
        "ANTHROPIC_API_KEY",
        "A=B",
        "",
    ] {
        assert!(env_write_denied(denied).is_some(), "{denied}");
    }
    for ok in ["PATH", "JAVA_HOME", "EDITOR", "MOJA_ZMIENNA"] {
        assert_eq!(env_write_denied(ok), None, "{ok}");
    }
    assert!(check_env_value("x").is_ok());
    assert!(check_env_value("a\0b").is_err());
}

#[test]
fn xpath_is_built_from_checked_values() {
    let q = EventQuery {
        log: EventLogName::System,
        min_level: Some(EventLevel::Error),
        provider: Some("Service Control Manager".into()),
        since_ms: Some(3_600_000),
        max: 10,
    };
    assert_eq!(
        event_xpath(&q).unwrap(),
        "*[System[(Level=1 or Level=2) and Provider[@Name='Service Control Manager'] and \
         TimeCreated[timediff(@SystemTime) <= 3600000]]]"
    );
    let all = EventQuery {
        min_level: Some(EventLevel::Information),
        provider: None,
        since_ms: None,
        ..q.clone()
    };
    assert!(event_xpath(&all).unwrap().contains("Level=0"));
    for evil in ["x' or '1'='1", "a]|//*[b", "Prov\"", "a\nb", ""] {
        let bad = EventQuery {
            provider: Some(evil.into()),
            ..q.clone()
        };
        assert!(event_xpath(&bad).is_err(), "{evil}");
    }
    assert_eq!(EventLogName::Application.channel(), "Application");
    assert_eq!(EventLevel::from_value(0), EventLevel::Information);
    assert_eq!(EventLevel::from_value(2), EventLevel::Error);
}

#[test]
fn download_names() {
    for (raw, want) in [
        ("raport.pdf", "raport.pdf"),
        ("../../AppData/evil.exe", "evil.exe"),
        (r"..\..\Startup\x.bat", "x.bat"),
        ("plik.txt:Zone.Identifier", "plik.txt_Zone.Identifier"),
        ("CON", "_CON"),
        ("nul.txt", "_nul.txt"),
        ("a. . ", "a"),
        ("", FALLBACK_NAME),
        ("...", FALLBACK_NAME),
        (".htaccess", "htaccess"),
        ("a<b>c|d?.txt", "a_b_c_d_.txt"),
    ] {
        assert_eq!(sanitize_file_name(raw), want, "{raw}");
    }
    let long = format!("{}.pdf", "x".repeat(500));
    let s = sanitize_file_name(&long);
    assert!(s.chars().count() <= MAX_DOWNLOAD_NAME && s.ends_with(".pdf"));
    assert!(is_executable_name("setup.EXE") && is_executable_name("a.lnk"));
    assert!(!is_executable_name("raport.pdf"));
    assert_eq!(numbered_name("raport.pdf", 2), "raport (2).pdf");
    assert_eq!(numbered_name("README", 3), "README (3)");
    assert_eq!(
        disposition_file_name("attachment; filename=\"a b.pdf\"").as_deref(),
        Some("a b.pdf")
    );
    assert_eq!(
        disposition_file_name(
            "attachment; filename=x.bin; filename*=UTF-8''%C5%BC%C3%B3%C5%82w.txt"
        )
        .as_deref(),
        Some("żółw.txt")
    );
    assert_eq!(disposition_file_name("inline"), None);
    let zone = zone_identifier("https://x.pl/a\r\n[ZoneTransfer]\r\nZoneId=0");
    assert!(zone.starts_with("[ZoneTransfer]\r\nZoneId=3\r\nHostUrl=https://x.pl/a[ZoneTransfer]"));
    assert_eq!(
        zone.matches("ZoneId=").count(),
        2,
        "wstrzyknięcie nie tworzy nowej linii"
    );
    assert_eq!(zone.lines().filter(|l| l.starts_with("ZoneId=")).count(), 1);
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    /// Dowolna nazwa od serwera → nazwa bez separatorów, ADS, znaków sterujących i zabronionych,
    /// bez kropki/spacji na końcu, niepusta, ≤ limit, nie nazwa urządzenia.
    #[test]
    fn sanitized_names_are_always_safe(raw in "\\PC{0,300}") {
        let s = sanitize_file_name(&raw);
        prop_assert!(!s.is_empty());
        prop_assert!(s.chars().count() <= MAX_DOWNLOAD_NAME);
        prop_assert!(!s.chars().any(|c| c.is_control()
            || matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|')));
        prop_assert!(!s.ends_with('.') && !s.ends_with(' ') && !s.starts_with('.'));
        let base = s.split('.').next().unwrap_or_default().trim().to_ascii_lowercase();
        prop_assert!(!["con", "prn", "aux", "nul", "com1", "lpt1"].contains(&base.as_str()));
    }

    /// Dostawca z dowolnymi znakami: albo odrzucony, albo XPath bez apostrofu i nawiasów z wejścia.
    #[test]
    fn provider_never_breaks_out_of_xpath(p in "\\PC{0,40}") {
        let q = EventQuery { log: EventLogName::Application, min_level: None,
            provider: Some(p.clone()), since_ms: None, max: 5 };
        if let Ok(x) = event_xpath(&q) {
            prop_assert!(!p.contains(['\'', '"', '[', ']', '(', ')', '|', '=']));
            prop_assert_eq!(x.matches('\'').count(), 2);
        }
    }
}
