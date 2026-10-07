//! `ProcessPort`: każdy proces w osobnym Job Object (`KILL_ON_JOB_CLOSE`, opcjonalne limity
//! pamięci, CPU i affinity — także do emulacji baseline, PLAN §3.5), zabijanie całego drzewa przez
//! `TerminateJobObject` (cel kill-switcha < 200 ms, PLAN §8.6), lista procesów, test „okno admina”.

mod cmdline;
mod exec;
#[cfg(windows)]
mod win;

/// Okno administratora na pierwszym planie (skróty globalne: naciśnięcie, którego hook nie
/// widział, przy oknie podniesionym jest fizyczne — UIPI blokuje `SendInput`; P2-04).
#[cfg(windows)]
pub(crate) use win::foreground_is_elevated;

use std::collections::BTreeMap;
use std::sync::{Mutex, MutexGuard};

use platform_contract::{PlatformError, ProcessHandle, ProcessPort, ProcessSpec, ProcessStatus};
use serde::{Deserialize, Serialize};

/// Limity Job Object nakładane na drzewo procesów.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct JobLimits {
    /// Limit pamięci całego drzewa w MB.
    pub memory_mb: Option<u32>,
    /// Twardy limit CPU w procentach całej maszyny (1–100).
    pub cpu_rate_percent: Option<u8>,
    /// Maska affinity (bit = procesor logiczny w grupie 0).
    pub affinity_mask: Option<u64>,
}

impl JobLimits {
    /// Limity emulujące baseline (PLAN §3.5): `logical_cpus` pierwszych procesorów + pamięć w MB.
    pub fn emulate(logical_cpus: u32, memory_mb: u32) -> Self {
        Self {
            memory_mb: Some(memory_mb),
            cpu_rate_percent: None,
            affinity_mask: Some(affinity_mask_for(logical_cpus)),
        }
    }

    /// Łączy limity: bardziej restrykcyjny wygrywa.
    pub fn merge(self, other: JobLimits) -> JobLimits {
        fn min_opt<T: Ord>(a: Option<T>, b: Option<T>) -> Option<T> {
            match (a, b) {
                (Some(x), Some(y)) => Some(x.min(y)),
                (x, y) => x.or(y),
            }
        }
        JobLimits {
            memory_mb: min_opt(self.memory_mb, other.memory_mb),
            cpu_rate_percent: min_opt(self.cpu_rate_percent, other.cpu_rate_percent),
            affinity_mask: match (self.affinity_mask, other.affinity_mask) {
                (Some(a), Some(b)) if a & b != 0 => Some(a & b),
                (Some(a), Some(_)) => Some(a),
                (a, b) => a.or(b),
            },
        }
    }
}

/// Maska `n` pierwszych procesorów logicznych (0 → 1 procesor, ≥ 64 → wszystkie).
pub fn affinity_mask_for(logical_cpus: u32) -> u64 {
    match logical_cpus {
        0 => 1,
        n if n >= 64 => u64::MAX,
        n => (1u64 << n) - 1,
    }
}

/// Wpis listy procesów systemu.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProcessInfo {
    /// Identyfikator procesu.
    pub pid: u32,
    /// Identyfikator rodzica.
    pub parent_pid: u32,
    /// Nazwa pliku wykonywalnego.
    pub name: String,
}

#[cfg(windows)]
type Child = win::Child;
#[cfg(not(windows))]
type Child = std::convert::Infallible;

/// Procesy uruchomione przez Alfę (uchwyt = PID; PID nie jest ponownie używany, dopóki trzymamy
/// uchwyt procesu). Zamknięcie portu zamyka Job Objects → system zabija pozostałe drzewa.
#[derive(Debug, Default)]
pub struct WinProcesses {
    defaults: JobLimits,
    children: Mutex<BTreeMap<u32, Child>>,
}

impl WinProcesses {
    /// Nowy port z limitami domyślnymi dla każdego procesu (np. emulacja baseline).
    pub fn new(defaults: JobLimits) -> Self {
        Self {
            defaults,
            children: Mutex::new(BTreeMap::new()),
        }
    }

    fn lock(&self) -> MutexGuard<'_, BTreeMap<u32, Child>> {
        self.children.lock().unwrap_or_else(|p| p.into_inner())
    }

    fn unknown(handle: ProcessHandle) -> PlatformError {
        PlatformError::UnknownResource(format!("proces {}", handle.0))
    }

    /// Uruchamia proces z dodatkowymi limitami (łączonymi z domyślnymi i z `spec.memory_limit_mb`).
    pub fn spawn_with_limits(
        &self,
        spec: ProcessSpec,
        limits: JobLimits,
    ) -> Result<ProcessHandle, PlatformError> {
        cmdline::validate_spec(&spec)?;
        let limits = self.defaults.merge(limits).merge(JobLimits {
            memory_mb: spec.memory_limit_mb,
            ..JobLimits::default()
        });
        #[cfg(windows)]
        {
            let line = cmdline::command_line(&spec.cmd, &spec.args);
            let child = win::spawn(&spec, &line, &limits)?;
            let pid = child.pid();
            self.lock().insert(pid, child);
            Ok(ProcessHandle(pid))
        }
        #[cfg(not(windows))]
        {
            let _ = limits;
            Err(PlatformError::Unsupported(
                "uruchamianie procesów w Job Object tylko na Windows".into(),
            ))
        }
    }

    /// Liczba aktywnych procesów w drzewie (Job Object) uruchomionym jako `handle`.
    pub fn tree_size(&self, handle: ProcessHandle) -> Result<u32, PlatformError> {
        let children = self.lock();
        let child = children
            .get(&handle.0)
            .ok_or_else(|| Self::unknown(handle))?;
        #[cfg(windows)]
        {
            child.active_processes()
        }
        #[cfg(not(windows))]
        {
            match *child {}
        }
    }

    /// Zapomina proces: zamyka uchwyty; `KILL_ON_JOB_CLOSE` zabija to, co jeszcze działa.
    pub fn release(&self, handle: ProcessHandle) -> Result<(), PlatformError> {
        self.lock()
            .remove(&handle.0)
            .map(drop)
            .ok_or_else(|| Self::unknown(handle))
    }

    /// Lista wszystkich procesów systemu (Toolhelp32).
    pub fn list(&self) -> Result<Vec<ProcessInfo>, PlatformError> {
        #[cfg(windows)]
        {
            win::list_processes()
        }
        #[cfg(not(windows))]
        {
            Err(PlatformError::Unsupported(
                "lista procesów tylko na Windows".into(),
            ))
        }
    }
}

impl ProcessPort for WinProcesses {
    fn spawn(&self, spec: ProcessSpec) -> Result<ProcessHandle, PlatformError> {
        self.spawn_with_limits(spec, JobLimits::default())
    }

    fn kill_tree(&self, handle: ProcessHandle) -> Result<(), PlatformError> {
        let mut children = self.lock();
        let child = children
            .get_mut(&handle.0)
            .ok_or_else(|| Self::unknown(handle))?;
        #[cfg(windows)]
        {
            child.kill_tree()
        }
        #[cfg(not(windows))]
        {
            match *child {}
        }
    }

    fn status(&self, handle: ProcessHandle) -> Result<ProcessStatus, PlatformError> {
        let children = self.lock();
        let child = children
            .get(&handle.0)
            .ok_or_else(|| Self::unknown(handle))?;
        #[cfg(windows)]
        {
            child.status()
        }
        #[cfg(not(windows))]
        {
            match *child {}
        }
    }

    fn foreground_is_elevated(&self) -> bool {
        #[cfg(windows)]
        {
            win::foreground_is_elevated()
        }
        #[cfg(not(windows))]
        {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn limits_merge_takes_the_stricter_value() {
        let defaults = JobLimits::emulate(12, 16_384);
        assert_eq!(defaults.affinity_mask, Some(0xFFF));
        let merged = defaults.merge(JobLimits {
            memory_mb: Some(512),
            cpu_rate_percent: Some(50),
            affinity_mask: Some(0b11),
        });
        assert_eq!(merged.memory_mb, Some(512));
        assert_eq!(merged.cpu_rate_percent, Some(50));
        assert_eq!(merged.affinity_mask, Some(0b11));
        let disjoint = JobLimits {
            affinity_mask: Some(0b1),
            ..JobLimits::default()
        }
        .merge(JobLimits {
            affinity_mask: Some(0b10),
            ..JobLimits::default()
        });
        assert_eq!(disjoint.affinity_mask, Some(0b1));
        assert_eq!(affinity_mask_for(0), 1);
        assert_eq!(affinity_mask_for(64), u64::MAX);
        assert_eq!(
            JobLimits::default().merge(JobLimits::default()),
            JobLimits::default()
        );
    }

    #[test]
    fn unknown_handles_and_invalid_specs_are_rejected() {
        let p = WinProcesses::new(JobLimits::default());
        let h = ProcessHandle(999_999);
        assert!(matches!(
            p.status(h),
            Err(PlatformError::UnknownResource(_))
        ));
        assert!(matches!(
            p.kill_tree(h),
            Err(PlatformError::UnknownResource(_))
        ));
        assert!(matches!(
            p.tree_size(h),
            Err(PlatformError::UnknownResource(_))
        ));
        assert!(matches!(
            p.release(h),
            Err(PlatformError::UnknownResource(_))
        ));
        let bad = ProcessSpec {
            cmd: PathBuf::from("relative.exe"),
            args: vec![],
            cwd: PathBuf::from("/"),
            integrity: Default::default(),
            memory_limit_mb: None,
        };
        assert!(matches!(p.spawn(bad), Err(PlatformError::InvalidPath(_))));
        if !cfg!(windows) {
            assert!(!p.foreground_is_elevated());
            assert!(p.list().is_err());
        }
    }
}
