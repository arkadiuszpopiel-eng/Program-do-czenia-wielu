//! Rozszerzenie `ProcessPort`: uruchomienie procesu z przechwyceniem stdout/stderr, limitem
//! czasu i rozmiaru wyjścia, jawnym środowiskiem (bez dziedziczenia sekretów) i anulowaniem
//! (docs/modules/tools-shell/SPEC.md, PLAN §8.7). Proces zawsze w Job Object (zabijane całe
//! drzewo przy przekroczeniu czasu, anulowaniu i kill-switchu).

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};

use crate::error::PlatformError;
use crate::process::{ProcessHandle, ProcessPort, ProcessSpec};

/// Maksymalny limit czasu jednego uruchomienia (1 h).
pub const MAX_EXEC_TIMEOUT_MS: u64 = 60 * 60 * 1000;
/// Maksymalny limit przechwyconego wyjścia na strumień (64 MiB).
pub const MAX_EXEC_OUTPUT_BYTES: usize = 64 * 1024 * 1024;

/// Zmienne środowiskowe przekazywane domyślnie powłokom (allowlista; reszta nie jest dziedziczona).
pub const DEFAULT_ENV_ALLOWLIST: [&str; 22] = [
    "PATH",
    "PATHEXT",
    "SYSTEMROOT",
    "SYSTEMDRIVE",
    "WINDIR",
    "COMSPEC",
    "TEMP",
    "TMP",
    "USERPROFILE",
    "HOMEDRIVE",
    "HOMEPATH",
    "LOCALAPPDATA",
    "APPDATA",
    "PROGRAMDATA",
    "PROGRAMFILES",
    "PROGRAMFILES(X86)",
    "COMMONPROGRAMFILES",
    "NUMBER_OF_PROCESSORS",
    "PROCESSOR_ARCHITECTURE",
    "OS",
    "LANG",
    "HOME",
];

/// Fragmenty nazw zmiennych uznawanych za sekrety — nigdy nie trafiają do procesu potomnego,
/// nawet gdy są na allowliście.
const SECRET_NAME_MARKERS: [&str; 9] = [
    "KEY",
    "TOKEN",
    "SECRET",
    "PASSWORD",
    "PASSWD",
    "CREDENTIAL",
    "AUTH",
    "COOKIE",
    "SESSION",
];

/// Czy nazwa zmiennej wygląda na sekret (`ANTHROPIC_API_KEY`, `GITHUB_TOKEN`, …).
pub fn is_secret_env_name(name: &str) -> bool {
    let upper = name.to_ascii_uppercase();
    SECRET_NAME_MARKERS.iter().any(|m| upper.contains(m))
}

/// Filtruje środowisko: tylko nazwy z allowlisty (bez rozróżniania wielkości liter), bez sekretów
/// i bez wartości zawierających NUL. Kolejność wejścia zachowana, duplikaty pominięte.
pub fn filter_env<I, K, V>(source: I, allowlist: &[&str]) -> Vec<(String, String)>
where
    I: IntoIterator<Item = (K, V)>,
    K: Into<String>,
    V: Into<String>,
{
    let mut out: Vec<(String, String)> = Vec::new();
    for (k, v) in source {
        let (k, v) = (k.into(), v.into());
        let allowed = allowlist.iter().any(|a| a.eq_ignore_ascii_case(&k));
        let duplicate = out.iter().any(|(n, _)| n.eq_ignore_ascii_case(&k));
        if allowed
            && !duplicate
            && !is_secret_env_name(&k)
            && !k.contains(['=', '\0'])
            && !v.contains('\0')
        {
            out.push((k, v));
        }
    }
    out
}

/// Specyfikacja uruchomienia z przechwyceniem wyjścia.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecSpec {
    /// Program, argumenty, katalog roboczy, integralność, limit pamięci (Job Object).
    pub process: ProcessSpec,
    /// Windows: surowa reszta wiersza poleceń (np. `/D /S /C "…"` dla `cmd.exe`, którego reguły
    /// cudzysłowów różnią się od MSVCRT). Gdy ustawiona, `process.args` są ignorowane.
    #[serde(default)]
    pub raw_args: Option<String>,
    /// Pełne środowisko procesu — **nic nie jest dziedziczone** (zob. [`filter_env`]).
    #[serde(default)]
    pub env: Vec<(String, String)>,
    /// Limit czasu (ms); po nim całe drzewo jest zabijane.
    pub timeout_ms: u64,
    /// Limit przechwyconego wyjścia na strumień (B); nadmiar jest liczony i odrzucany.
    pub max_output_bytes: usize,
}

impl ExecSpec {
    /// Walidacja wspólna dla implementacji (limity, środowisko).
    pub fn validate(&self) -> Result<(), PlatformError> {
        let invalid = |m: String| Err(PlatformError::Unsupported(m));
        if self.timeout_ms == 0 || self.timeout_ms > MAX_EXEC_TIMEOUT_MS {
            return invalid(format!(
                "limit czasu {} ms poza zakresem 1–{MAX_EXEC_TIMEOUT_MS}",
                self.timeout_ms
            ));
        }
        if self.max_output_bytes == 0 || self.max_output_bytes > MAX_EXEC_OUTPUT_BYTES {
            return invalid(format!(
                "limit wyjścia {} B poza zakresem 1–{MAX_EXEC_OUTPUT_BYTES}",
                self.max_output_bytes
            ));
        }
        if let Some((k, _)) = self
            .env
            .iter()
            .find(|(k, v)| k.is_empty() || k.contains(['=', '\0']) || v.contains('\0'))
        {
            return invalid(format!("niepoprawna zmienna środowiskowa `{k}`"));
        }
        if self.raw_args.as_deref().is_some_and(|a| a.contains('\0')) {
            return invalid("argumenty zawierają NUL".into());
        }
        Ok(())
    }
}

/// Jak zakończyło się uruchomienie.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", content = "code", rename_all = "snake_case")]
pub enum ExecTermination {
    /// Proces zakończył się sam z kodem.
    Exited(i32),
    /// Przekroczony limit czasu — drzewo zabite.
    TimedOut,
    /// Anulowane przez wywołującego — drzewo zabite.
    Cancelled,
    /// Zabite z zewnątrz (`kill_tree`, kill-switch).
    Killed,
}

/// Wynik uruchomienia.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecOutput {
    /// Uchwyt drzewa (do korelacji z rejestrem Job Objects).
    pub handle: Option<ProcessHandle>,
    /// Zakończenie.
    pub termination: ExecTermination,
    /// Przechwycone stdout (≤ limit).
    pub stdout: Vec<u8>,
    /// Przechwycone stderr (≤ limit).
    pub stderr: Vec<u8>,
    /// Ile bajtów stdout wyprodukował proces (także odrzuconych).
    pub stdout_total: u64,
    /// Ile bajtów stderr wyprodukował proces (także odrzuconych).
    pub stderr_total: u64,
    /// Czas trwania (ms).
    pub elapsed_ms: u64,
}

impl ExecOutput {
    /// Czy któreś wyjście zostało obcięte limitem.
    pub fn truncated(&self) -> bool {
        self.stdout_total > self.stdout.len() as u64 || self.stderr_total > self.stderr.len() as u64
    }
}

/// Bufor wyjścia z limitem: przechowuje pierwsze `limit` bajtów, liczy wszystkie.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CapturedStream {
    data: Vec<u8>,
    total: u64,
    limit: usize,
}

impl CapturedStream {
    /// Pusty bufor z limitem.
    pub fn new(limit: usize) -> Self {
        Self {
            data: Vec::new(),
            total: 0,
            limit,
        }
    }

    /// Dopisuje fragment (nadmiar ponad limit jest tylko liczony).
    pub fn push(&mut self, chunk: &[u8]) {
        self.total = self.total.saturating_add(chunk.len() as u64);
        let room = self.limit.saturating_sub(self.data.len());
        self.data.extend_from_slice(&chunk[..chunk.len().min(room)]);
    }

    /// Zawartość i liczba wszystkich bajtów.
    pub fn finish(self) -> (Vec<u8>, u64) {
        (self.data, self.total)
    }
}

type SpawnHook = Box<dyn Fn(ProcessHandle) + Send + Sync>;

/// Sterowanie uruchomieniem: anulowanie (flaga współdzielona między wątkami) i powiadomienie
/// o starcie procesu (np. rejestracja w tabeli Job Objects kill-switcha).
#[derive(Default)]
pub struct ExecControl {
    cancel: Arc<AtomicBool>,
    spawned: Mutex<Option<ProcessHandle>>,
    on_spawn: Option<SpawnHook>,
}

impl std::fmt::Debug for ExecControl {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ExecControl")
            .field("cancelled", &self.is_cancelled())
            .field("spawned", &self.spawned())
            .finish_non_exhaustive()
    }
}

impl ExecControl {
    /// Nowe sterowanie (bez anulowania).
    pub fn new() -> Self {
        Self::default()
    }

    /// Wywoływane raz, zaraz po starcie procesu (przed czekaniem na koniec).
    #[must_use]
    pub fn on_spawn(mut self, hook: impl Fn(ProcessHandle) + Send + Sync + 'static) -> Self {
        self.on_spawn = Some(Box::new(hook));
        self
    }

    /// Flaga anulowania (do ustawienia z innego wątku/zadania).
    pub fn cancel_flag(&self) -> Arc<AtomicBool> {
        self.cancel.clone()
    }

    /// Anuluje uruchomienie (implementacja zabija drzewo przy najbliższym sprawdzeniu, ≤ 50 ms).
    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::SeqCst);
    }

    /// Czy anulowano.
    pub fn is_cancelled(&self) -> bool {
        self.cancel.load(Ordering::SeqCst)
    }

    /// Uchwyt uruchomionego procesu (po starcie).
    pub fn spawned(&self) -> Option<ProcessHandle> {
        *self.spawned.lock().unwrap_or_else(|p| p.into_inner())
    }

    /// Dla implementacji: zapisuje uchwyt i wywołuje powiadomienie.
    pub fn notify_spawned(&self, handle: ProcessHandle) {
        *self.spawned.lock().unwrap_or_else(|p| p.into_inner()) = Some(handle);
        if let Some(hook) = &self.on_spawn {
            hook(handle);
        }
    }
}

/// Port uruchamiania z przechwyceniem wyjścia — rozszerzenie [`ProcessPort`] (ten sam rejestr
/// uchwytów: `kill_tree` z kill-switcha zabija także procesy uruchomione tutaj).
pub trait ExecPort: ProcessPort {
    /// Uruchamia proces w Job Object z jawnym środowiskiem, czeka na koniec, limit czasu albo
    /// anulowanie (wtedy zabija całe drzewo) i zwraca przechwycone wyjście (≤ limit na strumień).
    /// Blokuje wątek wywołujący — z kodu async wywoływać przez `spawn_blocking`.
    fn run_captured(
        &self,
        spec: ExecSpec,
        control: &ExecControl,
    ) -> Result<ExecOutput, PlatformError>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn spec() -> ExecSpec {
        ExecSpec {
            process: ProcessSpec {
                cmd: PathBuf::from("/bin/sh"),
                args: vec![],
                cwd: PathBuf::from("/"),
                integrity: Default::default(),
                memory_limit_mb: None,
            },
            raw_args: None,
            env: vec![],
            timeout_ms: 1000,
            max_output_bytes: 10,
        }
    }

    #[test]
    fn env_filter_drops_secrets_and_unknown() {
        let env = filter_env(
            [
                ("Path", "C:\\bin"),
                ("ANTHROPIC_API_KEY", "sk-ant"),
                ("TEMP", "C:\\t"),
                ("GITHUB_TOKEN", "ghp"),
                ("PATH", "dup"),
                ("RANDOM_VAR", "x"),
                ("TMP", "a\0b"),
            ],
            &DEFAULT_ENV_ALLOWLIST,
        );
        assert_eq!(
            env,
            vec![
                ("Path".to_owned(), "C:\\bin".to_owned()),
                ("TEMP".to_owned(), "C:\\t".to_owned())
            ]
        );
        assert!(is_secret_env_name("openai_api_key"));
        assert!(!is_secret_env_name("PATH"));
        let secret_allowed = filter_env([("MY_TOKEN", "x")], &["MY_TOKEN"]);
        assert!(secret_allowed.is_empty());
    }

    #[test]
    fn spec_validation() {
        assert!(spec().validate().is_ok());
        let mut s = spec();
        s.timeout_ms = 0;
        assert!(s.validate().is_err());
        let mut s = spec();
        s.max_output_bytes = MAX_EXEC_OUTPUT_BYTES + 1;
        assert!(s.validate().is_err());
        let mut s = spec();
        s.env = vec![("A=B".into(), "x".into())];
        assert!(s.validate().is_err());
        let mut s = spec();
        s.raw_args = Some("a\0".into());
        assert!(s.validate().is_err());
    }

    #[test]
    fn captured_stream_limits_and_counts() {
        let mut c = CapturedStream::new(4);
        c.push(b"ab");
        c.push(b"cdef");
        c.push(b"");
        let (data, total) = c.finish();
        assert_eq!(data, b"abcd");
        assert_eq!(total, 6);
        let out = ExecOutput {
            handle: None,
            termination: ExecTermination::Exited(0),
            stdout: data,
            stderr: vec![],
            stdout_total: total,
            stderr_total: 0,
            elapsed_ms: 1,
        };
        assert!(out.truncated());
    }

    #[test]
    fn control_cancel_and_spawn_hook() {
        let seen = Arc::new(Mutex::new(None));
        let s = seen.clone();
        let c = ExecControl::new().on_spawn(move |h| *s.lock().unwrap() = Some(h));
        assert!(!c.is_cancelled());
        c.notify_spawned(ProcessHandle(7));
        assert_eq!(c.spawned(), Some(ProcessHandle(7)));
        assert_eq!(*seen.lock().unwrap(), Some(ProcessHandle(7)));
        c.cancel_flag().store(true, Ordering::SeqCst);
        assert!(c.is_cancelled());
        assert!(format!("{c:?}").contains("cancelled: true"));
    }
}
