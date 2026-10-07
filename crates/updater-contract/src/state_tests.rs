use super::*;

fn v(s: &str) -> Version {
    Version::parse(s).unwrap()
}

fn now() -> DateTime<Utc> {
    DateTime::UNIX_EPOCH
}

#[test]
fn switch_rollback_and_mark_good() {
    let s = CurrentState::initial(v("1.0.0"), now());
    assert_eq!(s.switched(&v("1.0.0"), now()), s);
    let s2 = s.switched(&v("1.1.0"), now());
    assert_eq!(
        (s2.active.clone(), s2.previous.clone(), s2.pending),
        (v("1.1.0"), Some(v("1.0.0")), true)
    );
    let good = s2.marked_good(&v("1.1.0"), now());
    assert!(!good.pending);
    assert_eq!(s2.marked_good(&v("9.9.9"), now()), s2);
    let back = good.rolled_back(now()).unwrap();
    assert_eq!(
        (back.active.clone(), back.previous.clone()),
        (v("1.0.0"), Some(v("1.1.0")))
    );
    assert_eq!(s.rolled_back(now()), Err(UpdaterError::NoPrevious));
    let json = serde_json::to_string(&good).unwrap();
    assert_eq!(serde_json::from_str::<CurrentState>(&json).unwrap(), good);
}

#[test]
fn choose_active_then_previous_then_error() {
    let installed = vec![v("1.0.0"), v("1.1.0")];
    let s = CurrentState::initial(v("1.0.0"), now()).switched(&v("1.1.0"), now());
    let all = |_: &Version| true;
    let c = choose_version(Some(&s), &installed, &all).unwrap();
    assert_eq!((c.version, c.fallback), (v("1.1.0"), false));
    let no_active = |x: &Version| *x != v("1.1.0");
    let c = choose_version(Some(&s), &installed, &no_active).unwrap();
    assert_eq!((c.version, c.fallback), (v("1.0.0"), true));
    assert!(c.reason.is_some());
    let none = |_: &Version| false;
    assert!(matches!(
        choose_version(Some(&s), &installed, &none),
        Err(UpdaterError::NoUsableVersion { .. })
    ));
    let c = choose_version(None, &installed, &all).unwrap();
    assert_eq!((c.version, c.fallback), (v("1.1.0"), true));
    assert!(choose_version(None, &[], &all).is_err());
    let mut bad = s.clone();
    bad.bad.push(v("1.1.0"));
    assert_eq!(
        choose_version(Some(&bad), &installed, &all)
            .unwrap()
            .version,
        v("1.0.0")
    );
}

#[test]
fn crash_loop_policy() {
    let p = CrashPolicy::default();
    let base = CurrentState::initial(v("1.0.0"), now());
    let pending = base.switched(&v("1.1.0"), now());
    let quick = AppExit::Exited {
        code: 3,
        after_ms: 200,
    };
    // Wersja czekająca na `mark_good`: jedna szybka awaria → powrót do poprzedniej.
    let (s, d) = decide_exit(&pending, &v("1.1.0"), &quick, &p, true, now());
    assert_eq!(d, ExitDecision::FallBack { to: v("1.0.0") });
    assert_eq!(
        (s.active.clone(), s.previous.clone(), s.bad.clone()),
        (v("1.0.0"), None, vec![v("1.1.0")])
    );
    // Wersja dobra: pierwsza awaria → ponów, druga → powrót.
    let good = pending.marked_good(&v("1.1.0"), now());
    let (s1, d1) = decide_exit(&good, &v("1.1.0"), &quick, &p, true, now());
    assert_eq!((d1, s1.crashes), (ExitDecision::Retry, 1));
    let (_, d2) = decide_exit(
        &s1,
        &v("1.1.0"),
        &AppExit::FailedToStart { reason: "x".into() },
        &p,
        true,
        now(),
    );
    assert_eq!(d2, ExitDecision::FallBack { to: v("1.0.0") });
    // Zdrowe przypadki zerują licznik; awaria po oknie to nie problem startu.
    for exit in [
        AppExit::Running,
        AppExit::Exited {
            code: 0,
            after_ms: 10,
        },
        AppExit::Exited {
            code: 1,
            after_ms: 60_000,
        },
    ] {
        let (s, d) = decide_exit(&s1, &v("1.1.0"), &exit, &p, true, now());
        assert_eq!((d, s.crashes), (ExitDecision::Healthy, 0));
    }
    // Bez poprzedniej → poddaj się.
    let (_, d) = decide_exit(&pending, &v("1.1.0"), &quick, &p, false, now());
    assert!(matches!(d, ExitDecision::GiveUp { .. }));
    // Awaria wersji zapasowej (nie aktywnej): ponów, potem poddaj się.
    let (s, d) = decide_exit(&good, &v("1.0.0"), &quick, &p, true, now());
    assert_eq!(d, ExitDecision::Retry);
    let (_, d) = decide_exit(&s, &v("1.0.0"), &quick, &p, true, now());
    assert!(matches!(d, ExitDecision::GiveUp { .. }));
}

#[test]
fn prune_keeps_active_previous_and_staged() {
    let installed: Vec<Version> = ["0.9.0", "1.0.0", "1.1.0", "1.2.0", "1.3.0"]
        .iter()
        .map(|s| v(s))
        .collect();
    let s = CurrentState::initial(v("1.1.0"), now()).switched(&v("1.2.0"), now());
    let victims = prune_victims(&installed, Some(&s), 2);
    assert_eq!(
        victims,
        vec![v("0.9.0"), v("1.0.0")],
        "1.3.0 przygotowana — zostaje"
    );
    assert_eq!(
        prune_victims(&installed, None, 2),
        vec![v("0.9.0"), v("1.0.0"), v("1.1.0")]
    );
    let mut with_bad = CurrentState::initial(v("1.0.0"), now());
    with_bad.bad = vec![v("1.3.0")];
    let victims = prune_victims(&[v("0.9.0"), v("1.0.0")], Some(&with_bad), 1);
    assert_eq!(victims, vec![v("0.9.0")]);
}

#[test]
fn unconfirmed_pending_version_falls_back_at_once() {
    let s = CurrentState::initial(v("1.0.0"), now()).switched(&v("1.1.0"), now());
    let policy = CrashPolicy::default();
    assert_eq!(policy.confirm_ms, 300_000);
    let exit = AppExit::Unconfirmed {
        code: None,
        after_ms: policy.confirm_ms,
    };
    let (next, decision) = decide_exit(&s, &v("1.1.0"), &exit, &policy, true, now());
    assert_eq!(decision, ExitDecision::FallBack { to: v("1.0.0") });
    assert_eq!((next.active, next.bad), (v("1.0.0"), vec![v("1.1.0")]));
    let legacy: CrashPolicy =
        serde_json::from_str(r#"{"window_ms":1000,"max_quick_crashes":3}"#).unwrap();
    assert_eq!(legacy.confirm_ms, 300_000, "starszy zapis bez confirm_ms");
}
