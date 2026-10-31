//! Sonda parametrów komputera `pc-diag` na Windows ([`WinDiagProbe`]): skrypt PowerShell
//! (`probe.ps1`, przekazany jako `-EncodedCommand`) bez okna konsoli i bez profilu użytkownika,
//! limit czasu z zabiciem procesu, części wypisywane wierszami JSON i składane w [`stream`] (po limicie —
//! z częściami odczytanymi do tej pory), zapis parsowany w [`parse`]. Poza Windows sonda zwraca
//! [`DiagError::Unsupported`].

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

pub mod parse;
pub mod stream;

use std::io::Read;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use base64::Engine;
use pc_diag_contract::{DiagError, DiagProbe, Snapshot};

/// Skrypt sondy.
pub const PROBE_SCRIPT: &str = include_str!("probe.ps1");

/// Domyślny limit czasu odczytu (dziennik zdarzeń i liczniki SMART bywają wolne).
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(90);

/// Sonda przez PowerShell.
#[derive(Debug, Clone)]
pub struct WinDiagProbe {
    timeout: Duration,
}

impl Default for WinDiagProbe {
    fn default() -> Self {
        Self {
            timeout: DEFAULT_TIMEOUT,
        }
    }
}

/// `-EncodedCommand`: Base64 z UTF-16LE.
pub fn encode_command(script: &str) -> String {
    let bytes: Vec<u8> = script.encode_utf16().flat_map(u16::to_le_bytes).collect();
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

/// Ścieżka PowerShell 5.1 z katalogu systemu (nie z PATH — nie da się jej podmienić).
fn powershell() -> std::path::PathBuf {
    let root = std::env::var_os("SystemRoot").unwrap_or_else(|| r"C:\Windows".into());
    std::path::PathBuf::from(root).join(r"System32\WindowsPowerShell\v1.0\powershell.exe")
}

impl WinDiagProbe {
    /// Sonda z własnym limitem czasu.
    pub fn with_timeout(timeout: Duration) -> Self {
        Self { timeout }
    }

    /// Wyjście sondy i informacja, czy została zatrzymana po limicie czasu.
    fn run(&self) -> Result<(String, bool), DiagError> {
        let mut cmd = Command::new(powershell());
        cmd.args([
            "-NoLogo",
            "-NoProfile",
            "-NonInteractive",
            "-ExecutionPolicy",
            "Bypass",
            "-EncodedCommand",
            &encode_command(PROBE_SCRIPT),
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
        }
        let mut child = cmd
            .spawn()
            .map_err(|e| DiagError::Probe(format!("nie uruchomiono PowerShell: {e}")))?;
        let mut stdout = child
            .stdout
            .take()
            .ok_or_else(|| DiagError::Probe("brak wyjścia PowerShell".into()))?;
        let reader = std::thread::spawn(move || {
            let mut buf = Vec::new();
            let _ = stdout.read_to_end(&mut buf);
            buf
        });
        let start = Instant::now();
        let mut stopped = false;
        loop {
            match child.try_wait() {
                Ok(Some(_)) => break,
                Ok(None) if start.elapsed() > self.timeout => {
                    // Zabicie zamyka wyjście — wątek odczytu oddaje części wypisane do tej pory.
                    let _ = child.kill();
                    let _ = child.wait();
                    stopped = true;
                    break;
                }
                Ok(None) => std::thread::sleep(Duration::from_millis(100)),
                Err(e) => return Err(DiagError::Probe(e.to_string())),
            }
        }
        let bytes = reader
            .join()
            .map_err(|_| DiagError::Probe("wątek odczytu zakończył się błędem".into()))?;
        Ok((String::from_utf8_lossy(&bytes).into_owned(), stopped))
    }
}

impl DiagProbe for WinDiagProbe {
    fn snapshot(&self) -> Result<Snapshot, DiagError> {
        if !cfg!(windows) {
            return Err(DiagError::Unsupported(
                "zapis parametrów jest dostępny na Windows".into(),
            ));
        }
        let (out, stopped) = self.run()?;
        let json = stream::assemble(&out, stopped.then_some(self.timeout.as_secs()))
            .map_err(DiagError::Probe)?;
        parse::parse(&json, chrono::Utc::now()).map_err(DiagError::Probe)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encoded_command_is_utf16le_base64() {
        let e = encode_command("ą");
        let raw = base64::engine::general_purpose::STANDARD.decode(e).unwrap();
        assert_eq!(raw, vec![0x05, 0x01]);
        assert!(PROBE_SCRIPT.contains("ConvertTo-Json"));
        assert!(!PROBE_SCRIPT.contains("IPAddress") && !PROBE_SCRIPT.contains("MacAddress"));
    }
}
