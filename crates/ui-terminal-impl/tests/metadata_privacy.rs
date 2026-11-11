//! Cyfry z testowego sekretu mogą być legalnym PID. Metadane nie są treścią terminala.
#![allow(clippy::unwrap_used, clippy::expect_used)]
use core_bus_fake::FakeBus;
use platform_contract::{PlatformError, PseudoConsolePort, PtySession, PtySize, PtySpec};
use platform_fake::FakePty;
use std::{io::Read, path::PathBuf, sync::Arc, time::Duration};
use ui_terminal_contract::contract_tests::RecordingSink;
use ui_terminal_contract::{
    OpenRequest, TerminalConfig, TerminalProfile, TerminalPrograms, TerminalService, ui_only,
};
use ui_terminal_impl::TerminalManager;

struct PidPort(FakePty);
struct PidSession(Box<dyn PtySession>);
impl PseudoConsolePort for PidPort {
    fn spawn(&self, spec: &PtySpec) -> Result<Box<dyn PtySession>, PlatformError> {
        Ok(Box::new(PidSession(self.0.spawn(spec)?)))
    }
}
impl PtySession for PidSession {
    fn pid(&self) -> u32 {
        4417
    }
    fn take_output(&self) -> Result<Box<dyn Read + Send>, PlatformError> {
        self.0.take_output()
    }
    fn write_input(&self, data: &[u8]) -> Result<(), PlatformError> {
        self.0.write_input(data)
    }
    fn resize(&self, size: PtySize) -> Result<(), PlatformError> {
        self.0.resize(size)
    }
    fn exit_code(&self) -> Result<Option<i32>, PlatformError> {
        self.0.exit_code()
    }
    fn close(&self) -> Result<(), PlatformError> {
        self.0.close()
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn metadata_digits_do_not_mean_terminal_text_leaked() {
    const INPUT: &str = "Lista zakupow mleko 4417\r";
    let bus = Arc::new(FakeBus::default());
    let sink = Arc::new(RecordingSink::default());
    let manager = TerminalManager::new(
        Arc::new(PidPort(FakePty::with_banner(b"PS> ", true))),
        TerminalConfig {
            programs: TerminalPrograms {
                shell: Some(PathBuf::from(if cfg!(windows) {
                    r"C:\pwsh\pwsh.exe"
                } else {
                    "/usr/bin/pwsh"
                })),
                ..TerminalPrograms::default()
            },
            default_cwd: PathBuf::from("/"),
            max_sessions: 1,
        },
    )
    .with_env(vec![])
    .with_bus(bus.clone(), tokio::runtime::Handle::current());
    let id = manager
        .open(
            OpenRequest {
                profile: TerminalProfile::Shell,
                size: PtySize::default(),
                cwd: None,
            },
            &ui_only::user_gesture(),
            sink.clone(),
        )
        .unwrap();
    manager
        .input(id, INPUT.as_bytes(), &ui_only::user_gesture())
        .unwrap();
    assert!(sink.wait(|s| String::from_utf8_lossy(&s.output_of(id)).contains(INPUT)));
    manager.close(id).unwrap();
    tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            let events = bus.recorded();
            if ["terminal.opened", "terminal.closed", "terminal.exited"]
                .iter()
                .all(|name| events.iter().any(|e| e.kind.as_str() == *name))
            {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    let events = bus.recorded();
    let encoded: String = events
        .iter()
        .map(|e| format!("{} {}", e.kind.as_str(), e.payload))
        .collect();
    assert!(encoded.contains("4417"), "legalny PID, nie wyciek tekstu");
    assert!(!encoded.contains(INPUT.trim()));
    assert!(!encoded.contains("PS> "));
    for event in events {
        let mut keys: Vec<&str> = event
            .payload
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect();
        keys.sort_unstable();
        let expected: &[&str] = match event.kind.as_str() {
            "terminal.opened" => &["pid", "profile", "terminal"],
            "terminal.closed" => &["terminal"],
            "terminal.exited" => &["code", "terminal"],
            other => panic!("nieoczekiwane zdarzenie: {other}"),
        };
        assert_eq!(keys, expected, "tylko metadane cyklu życia");
    }
}
