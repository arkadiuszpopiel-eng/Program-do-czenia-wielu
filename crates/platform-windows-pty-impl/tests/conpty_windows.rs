//! ConPTY na Windows (CI `windows-latest`): wyjście VT procesu, kod wyjścia, zamknięcie zabija
//! drzewo procesów (Job Object), rozmiar.

#![cfg(windows)]
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::io::Read;
use std::path::PathBuf;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use platform_contract::{
    DEFAULT_ENV_ALLOWLIST, PseudoConsolePort, PtySession, PtySize, PtySpec, filter_env,
};
use platform_windows_pty_impl::WinPty;

fn cmd(args: &[&str]) -> PtySpec {
    let root = std::env::var("SystemRoot").unwrap_or_else(|_| r"C:\Windows".into());
    PtySpec {
        program: PathBuf::from(format!(r"{root}\System32\cmd.exe")),
        args: args.iter().map(|a| (*a).to_owned()).collect(),
        cwd: std::env::temp_dir(),
        env: filter_env(std::env::vars(), &DEFAULT_ENV_ALLOWLIST),
        size: PtySize {
            cols: 100,
            rows: 30,
        },
    }
}

fn pump(session: &dyn PtySession) -> mpsc::Receiver<Vec<u8>> {
    let mut out = session.take_output().unwrap();
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let mut buf = [0u8; 4096];
        while let Ok(n) = out.read(&mut buf) {
            if n == 0 || tx.send(buf[..n].to_vec()).is_err() {
                break;
            }
        }
    });
    rx
}

fn wait_for(rx: &mpsc::Receiver<Vec<u8>>, needle: &str, within: Duration) -> bool {
    let deadline = Instant::now() + within;
    let mut seen = Vec::new();
    while Instant::now() < deadline {
        if let Ok(chunk) = rx.recv_timeout(Duration::from_millis(100)) {
            seen.extend(chunk);
            if String::from_utf8_lossy(&seen).contains(needle) {
                return true;
            }
        }
    }
    false
}

#[test]
fn output_and_exit_code() {
    let s = WinPty
        .spawn(&cmd(&["/d", "/c", "echo alfa-pty-ok"]))
        .unwrap();
    let rx = pump(s.as_ref());
    assert!(wait_for(&rx, "alfa-pty-ok", Duration::from_secs(15)));
    let deadline = Instant::now() + Duration::from_secs(15);
    while s.exit_code().unwrap().is_none() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(50));
    }
    assert_eq!(s.exit_code().unwrap(), Some(0));
    assert!(s.take_output().is_err(), "wyjście tylko raz");
    s.close().unwrap();
    s.close().unwrap();
    assert!(s.write_input(b"x").is_err());
}

#[test]
fn close_kills_process_tree_and_input_reaches_process() {
    let s = WinPty.spawn(&cmd(&["/d", "/q"])).unwrap();
    let rx = pump(s.as_ref());
    s.resize(PtySize { cols: 80, rows: 25 }).unwrap();
    s.write_input(b"echo wpisane-%COMSPEC:~0,0%ok\r\n").unwrap();
    assert!(wait_for(&rx, "wpisane-ok", Duration::from_secs(15)));
    s.write_input(b"ping -n 60 127.0.0.1\r\n").unwrap();
    std::thread::sleep(Duration::from_millis(500));
    let started = Instant::now();
    s.close().unwrap();
    while s.exit_code().unwrap().is_none() {
        assert!(
            started.elapsed() < Duration::from_secs(5),
            "drzewo nie zostało zabite"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
}
