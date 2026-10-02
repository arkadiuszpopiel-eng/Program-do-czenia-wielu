//! Terminal w aplikacji na atrapie pseudokonsoli: strumień tylko w kanale UI (test szpiegowski —
//! token z ekranu logowania i wpisane hasło nie trafiają do zdarzeń magistrali ani `Debug`),
//! wejście base64 z gestu komendy, programy profili, zamknięcie zabija drzewo procesów.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use app_api::dto::{TerminalFrame, TerminalProfileId};
use app_terminal::{FrameSink, Programs, TerminalApp};
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use core_bus_fake::FakeBus;
use platform_fake::FakePty;
use ui_terminal_contract::TerminalPrograms;

const TOKEN: &str = "sk-ant-oat01-SEKRET-Z-EKRANU-LOGOWANIA";
const PASSWORD: &str = "moje-tajne-haslo-123";

#[derive(Default)]
struct Collect(Mutex<Vec<TerminalFrame>>);

impl FrameSink for Collect {
    fn send(&self, frame: TerminalFrame) {
        self.0.lock().unwrap().push(frame);
    }
}

impl Collect {
    fn text(&self) -> String {
        let mut out = Vec::new();
        for f in self.0.lock().unwrap().iter() {
            if let TerminalFrame::Output { data_b64 } = f {
                out.extend(STANDARD.decode(data_b64).unwrap());
            }
        }
        String::from_utf8_lossy(&out).into_owned()
    }

    fn exited(&self) -> bool {
        self.0
            .lock()
            .unwrap()
            .iter()
            .any(|f| matches!(f, TerminalFrame::Exit { .. }))
    }
}

fn programs(claude: bool) -> Programs {
    Programs::fixed(TerminalPrograms {
        shell: Some(PathBuf::from("/usr/bin/pwsh")),
        cmd: None,
        claude: claude.then(|| PathBuf::from("/usr/bin/claude")),
        codex: None,
    })
}

async fn eventually(mut check: impl FnMut() -> bool) {
    for _ in 0..200 {
        if check() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    panic!("warunek nie spełniony w czasie");
}

#[tokio::test(flavor = "multi_thread")]
async fn stream_goes_only_to_the_channel() {
    let banner = format!("Zaloguj się: {TOKEN}\r\n");
    let pty = Arc::new(FakePty::with_banner(banner.as_bytes(), true));
    let bus = Arc::new(FakeBus::default());
    let app = TerminalApp::new(Some(pty.clone()), programs(true), PathBuf::from("/"))
        .with_bus(bus.clone(), tokio::runtime::Handle::current());
    let sink = Arc::new(Collect::default());
    let opened = app
        .open(TerminalProfileId::ClaudeLogin, 80, 24, None, sink.clone())
        .unwrap();
    assert_eq!(opened.profile, TerminalProfileId::ClaudeLogin);
    assert!(opened.alive);
    eventually(|| sink.text().contains(TOKEN)).await;
    app.input(opened.id, &STANDARD.encode(PASSWORD)).unwrap();
    assert_eq!(pty.input_of(opened.pid), PASSWORD.as_bytes());
    eventually(|| sink.text().contains(PASSWORD)).await;
    app.resize(opened.id, 120, 40).unwrap();
    assert_eq!(app.list().len(), 1);
    let events: String = bus
        .recorded()
        .iter()
        .map(|e| serde_json::to_string(&**e).unwrap())
        .collect();
    assert!(events.contains("terminal.opened"));
    assert!(!events.contains(TOKEN) && !events.contains(PASSWORD));
    let shown = format!("{app:?}");
    assert!(!shown.contains(TOKEN) && !shown.contains(PASSWORD));
    app.close(opened.id).unwrap();
    assert!(pty.tree_killed(opened.pid));
    assert!(app.list().is_empty());
}

#[tokio::test(flavor = "multi_thread")]
async fn exit_is_reported_and_bad_input_is_rejected() {
    let pty = Arc::new(FakePty::with_banner(b"PS> ", false));
    let app = TerminalApp::new(Some(pty.clone()), programs(false), PathBuf::from("/"));
    let sink = Arc::new(Collect::default());
    let s = app
        .open(
            TerminalProfileId::Shell,
            80,
            24,
            Some("/".into()),
            sink.clone(),
        )
        .unwrap();
    assert!(app.input(s.id, "%%% nie base64").is_err());
    assert!(app.input(999, &STANDARD.encode("x")).is_err());
    assert!(app.resize(s.id, 1, 1).is_err(), "rozmiar poza zakresem");
    pty.exit(s.pid, 0);
    eventually(|| sink.exited()).await;
    let missing = app
        .open(TerminalProfileId::ClaudeLogin, 80, 24, None, sink.clone())
        .unwrap_err();
    assert!(
        missing.message.contains("nie jest zainstalowany"),
        "{missing:?}"
    );
    assert!(
        app.open(TerminalProfileId::Shell, 0, 24, None, sink)
            .is_err(),
        "zły rozmiar"
    );
}
