//! Testy kontraktu `tools-system`.

use serde_json::json;

use super::*;

#[test]
fn manifests_are_valid_and_least_privilege() {
    let ms = manifests();
    assert_eq!(ms.len(), 9);
    for m in &ms {
        m.validate().unwrap();
        assert!(
            check_args(&m.name, &sample_args(&m.name)).is_ok(),
            "{}",
            m.name
        );
        assert!(m.allowed_for(&["system".into()], false), "{}", m.name);
        if m.mutating {
            assert!(
                !m.allowed_for(&["system".into()], true),
                "rola tylko do odczytu"
            );
            assert!(!m.allowed_for(&["system.read".into()], false));
            assert_eq!(m.untrusted_output, None);
        } else {
            assert_eq!(m.capabilities, vec!["gui.control".to_owned()]);
            assert_eq!(m.untrusted_output, Some(TaintSource::File));
            assert!(m.allowed_for(&["system.read".into()], true));
        }
    }
    let by = |n: &str| ms.iter().find(|m| m.name == n).unwrap().clone();
    assert_eq!(by("system_process_kill").reversible, Reversibility::No);
    assert_eq!(
        by("system_service_control").capabilities,
        vec!["system.admin".to_owned()]
    );
    assert_eq!(by("system_env_set").reversible, Reversibility::Yes);
    assert!(!by("system_env").allowed_for(&["net".into(), "fs".into()], false));
    assert_eq!(
        sysinfo_capability().unwrap().to_string(),
        format!("gui.control({SYSINFO_APP})")
    );
}

#[test]
fn args_are_strict() {
    for (tool, bad) in [
        ("system_processes", json!({"limit": 0})),
        ("system_processes", json!({"limit": 501})),
        ("system_processes", json!({"name_contains": "a\nb"})),
        ("system_process_info", json!({"pid": -1})),
        ("system_process_kill", json!({"pid": 5})),
        ("system_process_kill", json!({"pid": 5, "name": " "})),
        ("system_services", json!({"state": "paused"})),
        (
            "system_service_control",
            json!({"name": "a/b", "action": "stop"}),
        ),
        (
            "system_service_control",
            json!({"name": "Spooler", "action": "delete"}),
        ),
        ("system_events", json!({"log": "security"})),
        (
            "system_events",
            json!({"log": "system", "provider": "x' or '1'='1"}),
        ),
        (
            "system_events",
            json!({"log": "system", "since_hours": 9999}),
        ),
        ("system_env", json!({"scope": "hklm"})),
        ("system_env_set", json!({"name": "", "value": "x"})),
        ("system_env_set", json!({"name": "A", "value": "a\u{0}b"})),
        ("system_status", json!({"x": 1})),
        ("system_nieznane", json!({})),
    ] {
        assert!(check_args(tool, &bad).is_err(), "{tool}: {bad}");
    }
    assert_eq!(
        check_args("system_env_set", &json!({"name": "PATH"})),
        Ok(()),
        "brak wartości = usunięcie"
    );
}

#[test]
fn config_defaults() {
    let c = SystemToolsConfig::default();
    assert!(c.max_list <= 500 && c.max_events <= 200 && c.max_undo > 0);
}

#[test]
fn policy_refusals() {
    for (tool, args) in [
        (
            "system_env_set",
            json!({"name": "APPDATA", "value": "C:\\x"}),
        ),
        (
            "system_env_set",
            json!({"name": "GITHUB_TOKEN", "value": "x"}),
        ),
        ("system_process_kill", json!({"pid": 4, "name": "System"})),
        (
            "system_process_kill",
            json!({"pid": 700, "name": "LSASS.EXE"}),
        ),
        (
            "system_service_control",
            json!({"name": "WinDefend", "action": "stop"}),
        ),
        (
            "system_service_control",
            json!({"name": "AlfaBroker", "action": "restart"}),
        ),
    ] {
        assert!(policy_refusal(tool, &args).is_some(), "{tool}: {args}");
    }
    for (tool, args) in [
        (
            "system_env_set",
            json!({"name": "PATH", "value": "C:\\bin"}),
        ),
        (
            "system_process_kill",
            json!({"pid": 4321, "name": "notepad.exe"}),
        ),
        (
            "system_service_control",
            json!({"name": "WinDefend", "action": "start"}),
        ),
        (
            "system_service_control",
            json!({"name": "Spooler", "action": "stop"}),
        ),
        ("system_processes", json!({})),
    ] {
        assert_eq!(policy_refusal(tool, &args), None, "{tool}: {args}");
    }
}
