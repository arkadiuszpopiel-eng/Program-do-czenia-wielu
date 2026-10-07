//! `ui-terminal` na atrapie pseudokonsoli: kontrakt, test szpiegowski (tokeny z ekranu
//! logowania i wpisywane hasło nie trafiają do zdarzeń, `Debug` ani środowiska), środowisko bez
//! sekretów Alfy, koniec procesu zgłoszony do UI, zamknięcie zabija drzewo procesów.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use core_bus_fake::FakeBus;
use platform_contract::PtySize;
use platform_fake::FakePty;
use ui_terminal_contract::contract_tests::{RecordingSink, run_all};
use ui_terminal_contract::{
    OpenRequest, TerminalConfig, TerminalError, TerminalProfile, TerminalPrograms, TerminalService,
    ui_only,
};
use ui_terminal_impl::TerminalManager;

const TOKEN: &str = "sk-ant-oat01-SEKRETNY-TOKEN-Z-EKRANU-LOGOWANIA";
const PASSWORD: &str = "moje-tajne-haslo-123";

fn config() -> TerminalConfig {
    TerminalConfig {
        programs: TerminalPrograms {
            shell: Some(PathBuf::from(if cfg!(windows) {
                r"C:\pwsh\pwsh.exe"
            } else {
                "/usr/bin/pwsh"
            })),
            cmd: None,
            claude: Some(PathBuf::from(if cfg!(windows) {
                r"C:\npm\claude.exe"
            } else {
                "/usr/bin/claude"
            })),
            codex: Some(PathBuf::from(if cfg!(windows) {
                r"C:\npm\codex.exe"
            } else {
                "/usr/bin/codex"
            })),
        },
        default_cwd: PathBuf::from("/"),
        max_sessions: 2,
    }
}

#[test]
fn contract_suite() {
    let pty = Arc::new(FakePty::with_banner(b"PS> ", true));
    let manager = TerminalManager::new(pty, config());
    run_all(&manager, 2);
}

#[tokio::test(flavor = "multi_thread")]
async fn spy_nothing_from_the_stream_leaks() {
    let banner = format!("Zaloguj się: {TOKEN}\r\n");
    let pty = Arc::new(FakePty::with_banner(banner.as_bytes(), true));
    let bus = Arc::new(FakeBus::default());
    let manager = TerminalManager::new(pty.clone(), config())
        .with_env(vec![
            ("PATH".into(), "/bin".into()),
            ("ANTHROPIC_API_KEY".into(), "sk-ant-api03-KLUCZ".into()),
            ("GITHUB_TOKEN".into(), "ghp_x".into()),
            ("USERPROFILE".into(), "/home/ala".into()),
        ])
        .with_bus(bus.clone(), tokio::runtime::Handle::current());
    let sink = Arc::new(RecordingSink::default());
    let g = ui_only::user_gesture();
    let req = OpenRequest {
        profile: TerminalProfile::ClaudeLogin,
        size: PtySize::default(),
        cwd: None,
    };
    let id = manager.open(req, &g, sink.clone()).unwrap();
    manager
        .input(id, format!("{PASSWORD}\r").as_bytes(), &g)
        .unwrap();
    assert!(
        sink.wait(|s| String::from_utf8_lossy(&s.output_of(id)).contains(PASSWORD)),
        "strumień płynie do UI"
    );
    assert!(String::from_utf8_lossy(&sink.output_of(id)).contains(TOKEN));
    let spec = pty.spawned().remove(0);
    assert!(
        spec.env
            .iter()
            .all(|(k, _)| k != "ANTHROPIC_API_KEY" && k != "GITHUB_TOKEN"),
        "{:?}",
        spec.env
    );
    assert!(
        spec.env
            .iter()
            .any(|(k, v)| k == "TERM" && v == "xterm-256color")
    );
    let debug = format!("{manager:?} {:?}", manager.list());
    manager.close(id).unwrap();
    assert!(sink.wait(|s| s.exited(id)));
    tokio::time::sleep(Duration::from_millis(100)).await;
    let events: String = bus
        .recorded()
        .iter()
        .map(|e| format!("{} {}", e.kind.as_str(), e.payload))
        .collect();
    assert!(
        events.contains("terminal.opened") && events.contains("terminal.closed"),
        "{events}"
    );
    for secret in [TOKEN, PASSWORD, "KLUCZ"] {
        assert!(!events.contains(secret), "zdarzenia: {secret}");
        assert!(!debug.contains(secret), "Debug: {secret}");
    }
}

#[test]
fn close_kills_tree_and_process_exit_reaches_ui() {
    let pty = Arc::new(FakePty::with_banner(b"", false));
    let manager = TerminalManager::new(pty.clone(), config());
    let sink = Arc::new(RecordingSink::default());
    let g = ui_only::user_gesture();
    let req = |p| OpenRequest {
        profile: p,
        size: PtySize {
            cols: 100,
            rows: 30,
        },
        cwd: Some(PathBuf::from("/tmp")),
    };
    let a = manager
        .open(req(TerminalProfile::CodexLogin), &g, sink.clone())
        .unwrap();
    let spec = pty.spawned().remove(0);
    assert_eq!(spec.args, vec!["login".to_owned()]);
    assert_eq!(spec.cwd, PathBuf::from("/tmp"));
    let pid = manager.list()[0].pid;
    pty.exit(pid, 0);
    assert!(sink.wait(|s| s.exited(a)), "koniec procesu zgłoszony do UI");
    assert!(sink.wait(|_| !manager.list()[0].alive));
    manager.close(a).unwrap();
    assert!(pty.tree_killed(pid));
    assert_eq!(
        manager.open(req(TerminalProfile::Cmd), &g, sink.clone()),
        Err(TerminalError::ProgramMissing("Cmd".into()))
    );
    let b = manager
        .open(req(TerminalProfile::Shell), &g, sink.clone())
        .unwrap();
    let pid_b = manager.list()[0].pid;
    drop(manager);
    assert!(
        pty.tree_killed(pid_b),
        "porzucenie menedżera zabija drzewa ({b:?})"
    );
}
