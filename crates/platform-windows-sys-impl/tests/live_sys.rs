//! Testy na żywo portu systemu i kwarantanny (self-hosted Windows) — `#[ignore]`, ręcznie:
//! `cargo test -p platform-windows-sys-impl --test live_sys -- --ignored`.

#![allow(clippy::unwrap_used, clippy::expect_used)]

#[cfg(windows)]
mod live {
    use platform_apps_contract::{
        DownloadStore, EnvScope, EventLevel, EventLogName, EventQuery, ProcessIdentity,
        ServiceCommand, SysError, SysPort,
    };
    use platform_contract::TargetGuard;
    use platform_windows_sys_impl::{DiskDownloads, WinSys};

    fn sys() -> WinSys {
        WinSys::new(TargetGuard::baseline().with_pids([std::process::id()]))
    }

    #[test]
    #[ignore = "wymaga Windows (self-hosted)"]
    fn processes_and_own_process_protection() {
        let s = sys();
        let all = s.processes().unwrap();
        let me = all.iter().find(|p| p.pid == std::process::id()).unwrap();
        assert_eq!(me.own, Some(true));
        let d = s.process(me.pid).unwrap();
        assert!(d.path.is_some() && d.started_ms.is_some());
        let err = s.terminate(&ProcessIdentity::of(&d)).unwrap_err();
        assert!(matches!(err, SysError::Protected(_)), "{err:?}");
        let lsass = all
            .iter()
            .find(|p| p.image.eq_ignore_ascii_case("lsass.exe"));
        if let Some(p) = lsass {
            let id = ProcessIdentity {
                pid: p.pid,
                image: p.image.clone(),
                started_ms: None,
            };
            assert!(matches!(s.terminate(&id), Err(SysError::Protected(_))));
        }
    }

    #[test]
    #[ignore = "wymaga Windows (self-hosted)"]
    fn services_events_and_env() {
        let s = sys();
        let svcs = s.services().unwrap();
        assert!(svcs.iter().any(|v| v.name.eq_ignore_ascii_case("EventLog")));
        assert!(matches!(
            s.control_service("EventLog", ServiceCommand::Stop, 1_000),
            Err(SysError::Protected(_))
        ));
        let ev = s
            .events(&EventQuery {
                log: EventLogName::System,
                min_level: Some(EventLevel::Information),
                provider: None,
                since_ms: Some(30 * 24 * 3_600_000),
                max: 5,
            })
            .unwrap();
        assert!(ev.len() <= 5);
        assert!(ev.windows(2).all(|w| w[0].time_ms >= w[1].time_ms));
        let user = s.env(EnvScope::User).unwrap();
        assert!(user.iter().any(|v| v.name.eq_ignore_ascii_case("TEMP")));
        let name = "ALFA_TEST_ZMIENNA_ZAPIS";
        assert!(
            s.set_user_env(name, Some("1")).is_err(),
            "prefiks ALFA zabroniony"
        );
        let name = "ZMIENNA_TESTOWA_ALFY_LIVE";
        let prev = s.set_user_env(name, Some("1")).unwrap();
        assert_eq!(s.user_env_value(name).unwrap().as_deref(), Some("1"));
        s.set_user_env(name, prev.as_deref()).unwrap();
    }

    #[test]
    #[ignore = "wymaga Windows (self-hosted)"]
    fn download_gets_mark_of_the_web() {
        let dir = std::env::temp_dir().join("alfa-kwarantanna-live");
        let mut sink = DiskDownloads.begin(&dir, "plik.txt").unwrap();
        sink.write(b"x").unwrap();
        let path = sink.commit("https://example.com/plik.txt").unwrap();
        let mut zone = path.as_os_str().to_os_string();
        zone.push(":Zone.Identifier");
        let text = std::fs::read_to_string(zone).unwrap();
        assert!(text.contains("ZoneId=3"));
        std::fs::remove_file(path).unwrap();
    }
}
