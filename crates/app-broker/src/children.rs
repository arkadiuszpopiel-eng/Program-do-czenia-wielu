//! Procesy Jądra uruchamiane przez aplikację (tryb przenośny `alfa-broker --console`, zawsze
//! `alfa-watchdog`): „linia życia” na stdin (proces kończy się, gdy aplikacja zniknie — także po
//! awarii, bez sierot trzymających nazwę potoku), stdout/stderr do aplikacji (komunikaty watchdoga,
//! dziennik Brokera). Bez okna konsoli. Uchwyt `Child` zamiast PID — bez wyścigu ponownego użycia.

use std::io::{BufRead, BufReader, Read};
use std::path::PathBuf;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::{Arc, Mutex, MutexGuard};

/// Co uruchomić.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChildSpec {
    /// Ścieżka bezwzględna obrazu (katalog wersji aplikacji).
    pub image: PathBuf,
    /// Argumenty (bez sekretów — nic poufnego nie trafia do wiersza poleceń).
    pub args: Vec<String>,
}

/// Uruchomiony proces potomny.
pub trait ChildProc: Send {
    /// PID (do sprawdzenia serwera potoku).
    fn pid(&self) -> u32;
    /// Czy nadal działa (po uchwycie).
    fn running(&mut self) -> bool;
    /// Strumień wyjścia (raz).
    fn take_stdout(&mut self) -> Option<Box<dyn Read + Send>>;
    /// Strumień błędów (raz).
    fn take_stderr(&mut self) -> Option<Box<dyn Read + Send>>;
    /// Zatrzymuje proces (zamknięcie aplikacji, ponowne uruchomienie).
    fn kill(&mut self);
}

/// Uruchamianie procesów potomnych.
pub trait Spawner: Send + Sync {
    /// Uruchamia proces z linią życia na stdin.
    fn spawn(&self, spec: &ChildSpec) -> Result<Box<dyn ChildProc>, String>;
}

/// `std::process` (każdy system; na Windows bez okna konsoli).
#[derive(Debug, Clone, Copy, Default)]
pub struct StdSpawner;

struct StdChild {
    child: Child,
    // Linia życia: koniec zapisu zamyka się razem z procesem aplikacji (także przy awarii).
    _lifeline: Option<ChildStdin>,
}

impl ChildProc for StdChild {
    fn pid(&self) -> u32 {
        self.child.id()
    }

    fn running(&mut self) -> bool {
        matches!(self.child.try_wait(), Ok(None))
    }

    fn take_stdout(&mut self) -> Option<Box<dyn Read + Send>> {
        self.child
            .stdout
            .take()
            .map(|s| Box::new(s) as Box<dyn Read + Send>)
    }

    fn take_stderr(&mut self) -> Option<Box<dyn Read + Send>> {
        self.child
            .stderr
            .take()
            .map(|s| Box::new(s) as Box<dyn Read + Send>)
    }

    fn kill(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl Drop for StdChild {
    fn drop(&mut self) {
        // Proces Jądra uruchomiony przez aplikację nie przeżywa jej zamknięcia.
        self.kill();
    }
}

impl Spawner for StdSpawner {
    fn spawn(&self, spec: &ChildSpec) -> Result<Box<dyn ChildProc>, String> {
        if !spec.image.is_absolute() {
            return Err(format!(
                "{}: wymagana ścieżka bezwzględna",
                spec.image.display()
            ));
        }
        let mut cmd = Command::new(&spec.image);
        cmd.args(&spec.args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        if let Some(dir) = spec.image.parent() {
            cmd.current_dir(dir);
        }
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
        }
        let mut child = cmd
            .spawn()
            .map_err(|e| format!("{}: {e}", spec.image.display()))?;
        let lifeline = child.stdin.take();
        Ok(Box::new(StdChild {
            child,
            _lifeline: lifeline,
        }))
    }
}

/// Ostatnia linia dziennika procesu (powód zakończenia w UI).
#[derive(Debug, Clone, Default)]
pub struct LastLine(Arc<Mutex<Option<String>>>);

impl LastLine {
    fn lock(&self) -> MutexGuard<'_, Option<String>> {
        self.0.lock().unwrap_or_else(|p| p.into_inner())
    }

    /// Ostatnia linia.
    pub fn get(&self) -> Option<String> {
        self.lock().clone()
    }

    fn set(&self, line: &str) {
        *self.lock() = Some(line.chars().take(300).collect());
    }
}

/// Czyta linie strumienia w osobnym wątku: każda trafia do `on_line`, ostatnia — do `last`.
pub fn pump_lines(
    name: &str,
    stream: Box<dyn Read + Send>,
    last: LastLine,
    mut on_line: impl FnMut(&str) + Send + 'static,
) {
    let thread = format!("alfa-{name}-io");
    let spawned = std::thread::Builder::new().name(thread).spawn(move || {
        for line in BufReader::new(stream).lines() {
            let Ok(line) = line else { break };
            let line = line.trim_end();
            if line.is_empty() {
                continue;
            }
            last.set(line);
            on_line(line);
        }
    });
    if let Err(e) = spawned {
        tracing::error!(error = %e, "wątek odczytu procesu Jądra nie wystartował");
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::sync::mpsc;
    use std::time::Duration;

    #[test]
    fn std_child_streams_lines_and_dies_with_lifeline() {
        let spec = ChildSpec {
            image: PathBuf::from("/bin/sh"),
            // `cat` kończy się dopiero, gdy linia życia (stdin) zostanie zamknięta.
            args: vec![
                "-c".into(),
                "echo gotowy; echo blad >&2; cat >/dev/null".into(),
            ],
        };
        let mut child = StdSpawner.spawn(&spec).unwrap();
        assert!(child.pid() > 0);
        let (tx, rx) = mpsc::channel();
        let last = LastLine::default();
        let out = child.take_stdout().unwrap();
        pump_lines("test", out, last.clone(), move |l| {
            let _ = tx.send(l.to_owned());
        });
        assert_eq!(rx.recv_timeout(Duration::from_secs(5)).unwrap(), "gotowy");
        let err_last = LastLine::default();
        pump_lines(
            "test-err",
            child.take_stderr().unwrap(),
            err_last.clone(),
            |_| {},
        );
        assert!(child.running());
        drop(child);
        assert_eq!(last.get().as_deref(), Some("gotowy"));
        let relative = ChildSpec {
            image: PathBuf::from("sh"),
            args: Vec::new(),
        };
        assert!(StdSpawner.spawn(&relative).is_err());
    }
}
