//! `tools-system`: procesy — kontrakt, lista z oznaczeniem chronionych, zakończenie wyłącznie
//! procesu użytkownika przez `gui.control(<obraz>)`; Alfa, jej drzewo, Broker, watchdog, alias
//! 8.3, katalog Alfy, procesy krytyczne, cudze i podniesione → odmowa bez wywołania `terminate`
//! i bez tokenu; wyścig ponownego użycia PID-u → odmowa portu.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use common::{LAUNCHER, NOTEPAD, ctx, harness};
use platform_apps_fake::{SysCall, fake_process};
use risk_classifier_contract::KernelRule;
use safety_broker_fake::ScriptedDecision;
use serde_json::json;
use tools_common_contract::{DenialReason, ToolErrorKind, ToolStatus, Toolset};

#[tokio::test]
async fn contract_suite() {
    let h = harness(true);
    tools_system_contract::contract_tests::run_all(&h.tools.tools()).await;
}

#[tokio::test]
async fn list_marks_protected_and_is_untrusted() {
    let h = harness(true);
    let out = h.tool("system_processes").call(json!({}), &ctx()).await;
    assert!(out.is_ok(), "{}", out.text);
    assert!(out.untrusted.is_some());
    assert!(h.tainted(), "taint zgłoszony Brokerowi");
    let procs = out.data["processes"].as_array().unwrap();
    let protected = |pid: u32| {
        procs
            .iter()
            .find(|p| p["pid"] == pid)
            .map(|p| p["protected"] == true)
            .unwrap()
    };
    for pid in [std::process::id(), 5000, LAUNCHER, 901, 902, 903, 904, 700] {
        assert!(protected(pid), "PID {pid}");
    }
    assert!(!protected(NOTEPAD));
    assert_eq!(h.issued()[0].0, "gui.control");
    assert_eq!(h.issued()[0].1, "system-info.exe");
    let filtered = h
        .tool("system_processes")
        .call(json!({"name_contains": "NOTE", "limit": 1}), &ctx())
        .await;
    assert_eq!(filtered.data["total"], 1);
}

#[tokio::test]
async fn details_hide_path_of_protected_process() {
    let h = harness(true);
    let me = h
        .tool("system_process_info")
        .call(json!({"pid": 905}), &ctx())
        .await;
    assert!(me.is_ok());
    assert_eq!(
        me.data["process"]["protected"], true,
        "obraz w katalogu Alfy"
    );
    assert!(me.data["path"].is_null());
    let np = h
        .tool("system_process_info")
        .call(json!({"pid": NOTEPAD}), &ctx())
        .await;
    assert!(np.data["path"].as_str().unwrap().ends_with("notepad.exe"));
    let missing = h
        .tool("system_process_info")
        .call(json!({"pid": 99999}), &ctx())
        .await;
    assert_eq!(
        missing.status,
        ToolStatus::Failed {
            error: ToolErrorKind::NotFound
        }
    );
}

#[tokio::test]
async fn kill_user_process_goes_through_broker_with_image() {
    let h = harness(true);
    let out = h
        .tool("system_process_kill")
        .call(json!({"pid": NOTEPAD, "name": "Notepad.exe"}), &ctx())
        .await;
    assert!(out.is_ok(), "{}", out.text);
    assert_eq!(h.sys.calls(), vec![SysCall::Terminated(NOTEPAD)]);
    assert!(
        h.issued()
            .iter()
            .any(|(c, s)| c == "gui.control" && s == "notepad.exe")
    );
    assert_eq!(out.untrusted, None);
}

#[tokio::test]
async fn kill_of_alfa_kernel_and_foreign_processes_is_refused_before_broker() {
    let h = harness(true);
    let me = std::process::id();
    for (pid, name, kernel) in [
        (me, "alfa.exe", true),
        (5000, "msedgewebview2.exe", true),
        (LAUNCHER, "alfa-launcher.exe", true),
        (901, "llama-server.exe", true),
        (902, "alfa-broker.exe", true),
        (903, "alfa-watchdog.exe", true),
        (904, "ALFA-B~1.EXE", true),
        (905, "helper.exe", true),
        (700, "lsass.exe", false),
        (4, "System", false),
        (6000, "chrome.exe", false),
        (6001, "regedit.exe", false),
    ] {
        let out = h
            .tool("system_process_kill")
            .call(json!({"pid": pid, "name": name}), &ctx())
            .await;
        let want = if kernel {
            DenialReason::KernelBlock {
                rule: KernelRule::GuiControlOfKernelProcess,
            }
        } else {
            DenialReason::Policy
        };
        assert_eq!(out.status, ToolStatus::Denied { reason: want }, "{name}");
    }
    assert!(h.sys.calls().is_empty(), "zero wywołań terminate");
    assert!(
        h.issued().iter().all(|(c, _)| c != "gui.control"),
        "żadnego tokenu dla chronionych: {:?}",
        h.issued()
    );
}

#[tokio::test]
async fn pid_reuse_is_detected() {
    let h = harness(true);
    let wrong_name = h
        .tool("system_process_kill")
        .call(json!({"pid": NOTEPAD, "name": "calc.exe"}), &ctx())
        .await;
    assert_eq!(
        wrong_name.status,
        ToolStatus::Failed {
            error: ToolErrorKind::NotFound
        }
    );
    let mut reborn = fake_process(NOTEPAD, 1, "notepad.exe");
    reborn.started_ms = Some(9_999_999);
    h.sys.replace_before_terminate(reborn);
    let raced = h
        .tool("system_process_kill")
        .call(json!({"pid": NOTEPAD, "name": "notepad.exe"}), &ctx())
        .await;
    assert_eq!(
        raced.status,
        ToolStatus::Failed {
            error: ToolErrorKind::NotFound
        },
        "{}",
        raced.text
    );
    assert!(raced.text.contains("ponownie"));
    assert!(h.sys.calls().is_empty());
}

#[tokio::test]
async fn broker_denial_or_timeout_means_no_kill() {
    let h = harness(false);
    h.broker.script(
        "tools-system.process_kill",
        ScriptedDecision::Deny(KernelRule::GuiControlOfKernelProcess),
    );
    let out = h
        .tool("system_process_kill")
        .call(json!({"pid": NOTEPAD, "name": "notepad.exe"}), &ctx())
        .await;
    assert!(matches!(out.status, ToolStatus::Denied { .. }));
    let h = harness(false);
    h.broker
        .script("tools-system.process_kill", ScriptedDecision::NeedsApproval);
    let out = h
        .tool("system_process_kill")
        .call(json!({"pid": NOTEPAD, "name": "notepad.exe"}), &ctx())
        .await;
    assert_eq!(
        out.status,
        ToolStatus::Denied {
            reason: DenialReason::ApprovalTimeout {
                approval: safety_broker_contract::ApprovalId(1)
            }
        },
        "{}",
        out.text
    );
    assert!(h.sys.calls().is_empty());
}

#[tokio::test]
async fn cancelled_run_kills_nothing() {
    let h = harness(true);
    let c = ctx();
    c.cancel.cancel();
    let out = h
        .tool("system_process_kill")
        .call(json!({"pid": NOTEPAD, "name": "notepad.exe"}), &c)
        .await;
    assert_eq!(out.status, ToolStatus::Cancelled);
    assert!(h.sys.calls().is_empty());
}
