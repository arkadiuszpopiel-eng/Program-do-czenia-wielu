//! Atrapa `ExecPort`: skryptowane wyniki poleceń bez uruchamiania procesów. Czas trwania jest
//! deklarowany (bez czekania) — limit czasu rozstrzyga się deterministycznie; tryb „zawieszony”
//! czeka (krótkimi krokami) na anulowanie, `kill_tree` albo limit czasu.

use std::collections::{BTreeMap, VecDeque};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use platform_contract::{
    CapturedStream, ExecControl, ExecOutput, ExecPort, ExecSpec, ExecTermination, PlatformError,
    ProcessHandle, ProcessPort, ProcessSpec, ProcessStatus,
};

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

/// Skutek uboczny „skryptu” (np. zmiana plików w `FakeFs`), wykonywany przed zwróceniem wyniku.
pub type ExecEffect = Arc<dyn Fn(&ExecSpec) + Send + Sync>;
type Matcher = Arc<dyn Fn(&ExecSpec) -> bool + Send + Sync>;

/// Skryptowany wynik uruchomienia.
#[derive(Clone, Default)]
pub struct FakeRun {
    /// Wyjście standardowe.
    pub stdout: Vec<u8>,
    /// Wyjście błędów.
    pub stderr: Vec<u8>,
    /// Kod wyjścia.
    pub exit_code: i32,
    /// Deklarowany czas trwania (ms) — porównywany z limitem, bez czekania.
    pub duration_ms: u64,
    /// Proces „wisi” do anulowania / `kill_tree` / limitu czasu (czas rzeczywisty).
    pub hang: bool,
    /// Skutek uboczny.
    pub effect: Option<ExecEffect>,
}

impl std::fmt::Debug for FakeRun {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FakeRun")
            .field("stdout", &String::from_utf8_lossy(&self.stdout))
            .field("exit_code", &self.exit_code)
            .field("duration_ms", &self.duration_ms)
            .field("hang", &self.hang)
            .finish_non_exhaustive()
    }
}

impl FakeRun {
    /// Sukces z danym stdout.
    pub fn ok(stdout: &str) -> Self {
        Self {
            stdout: stdout.as_bytes().to_vec(),
            duration_ms: 5,
            ..Self::default()
        }
    }

    /// Zakończenie z kodem i wyjściami.
    pub fn exit(code: i32, stdout: &str, stderr: &str) -> Self {
        Self {
            stdout: stdout.as_bytes().to_vec(),
            stderr: stderr.as_bytes().to_vec(),
            exit_code: code,
            duration_ms: 5,
            ..Self::default()
        }
    }

    /// Proces wiszący (do testów anulowania i kill-switcha).
    pub fn hanging() -> Self {
        Self {
            hang: true,
            ..Self::default()
        }
    }

    /// Ustawia deklarowany czas trwania.
    #[must_use]
    pub fn taking(mut self, ms: u64) -> Self {
        self.duration_ms = ms;
        self
    }

    /// Dodaje skutek uboczny.
    #[must_use]
    pub fn with_effect(mut self, effect: impl Fn(&ExecSpec) + Send + Sync + 'static) -> Self {
        self.effect = Some(Arc::new(effect));
        self
    }
}

/// Atrapa uruchamiania poleceń: kolejka jednorazowa → reguły → wynik domyślny.
#[derive(Default)]
pub struct FakeExec {
    queue: Mutex<VecDeque<FakeRun>>,
    rules: Mutex<Vec<(Matcher, FakeRun)>>,
    default: Mutex<Option<FakeRun>>,
    runs: Mutex<Vec<ExecSpec>>,
    procs: Mutex<BTreeMap<u32, ProcessStatus>>,
    next: Mutex<u32>,
    fail_spawn: Mutex<Option<PlatformError>>,
}

impl std::fmt::Debug for FakeExec {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FakeExec")
            .field("runs", &lock(&self.runs).len())
            .finish_non_exhaustive()
    }
}

/// Tekst polecenia do dopasowań: argumenty i surowa reszta wiersza.
pub fn command_text(spec: &ExecSpec) -> String {
    let mut parts = spec.process.args.clone();
    parts.extend(spec.raw_args.clone());
    parts.join(" ")
}

impl FakeExec {
    /// Pusta atrapa (wynik domyślny: kod 0, puste wyjście).
    pub fn new() -> Self {
        Self::default()
    }

    /// Wynik dla najbliższego uruchomienia (FIFO, przed regułami).
    pub fn push(&self, run: FakeRun) {
        lock(&self.queue).push_back(run);
    }

    /// Reguła trwała: pierwsza pasująca wygrywa.
    pub fn on(&self, matcher: impl Fn(&ExecSpec) -> bool + Send + Sync + 'static, run: FakeRun) {
        lock(&self.rules).push((Arc::new(matcher), run));
    }

    /// Reguła: polecenie zawiera fragment (bez rozróżniania wielkości liter).
    pub fn on_command(&self, needle: &str, run: FakeRun) {
        let needle = needle.to_lowercase();
        self.on(
            move |spec| command_text(spec).to_lowercase().contains(&needle),
            run,
        );
    }

    /// Wynik, gdy nic nie pasuje.
    pub fn set_default(&self, run: FakeRun) {
        *lock(&self.default) = Some(run);
    }

    /// Najbliższy start zakończy się błędem.
    pub fn fail_next_spawn(&self, error: PlatformError) {
        *lock(&self.fail_spawn) = Some(error);
    }

    /// Wszystkie przyjęte uruchomienia (do asercji: środowisko, argumenty, limity).
    pub fn runs(&self) -> Vec<ExecSpec> {
        lock(&self.runs).clone()
    }

    fn pick(&self, spec: &ExecSpec) -> FakeRun {
        if let Some(run) = lock(&self.queue).pop_front() {
            return run;
        }
        let rules = lock(&self.rules);
        if let Some((_, run)) = rules.iter().find(|(m, _)| m(spec)) {
            return run.clone();
        }
        drop(rules);
        lock(&self.default).clone().unwrap_or_default()
    }

    fn allocate(&self) -> ProcessHandle {
        let mut next = lock(&self.next);
        *next += 1;
        let handle = ProcessHandle(*next);
        lock(&self.procs).insert(handle.0, ProcessStatus::Running);
        handle
    }

    fn set_status(&self, handle: ProcessHandle, status: ProcessStatus) {
        lock(&self.procs).insert(handle.0, status);
    }

    fn wait_hanging(
        &self,
        handle: ProcessHandle,
        spec: &ExecSpec,
        control: &ExecControl,
    ) -> ExecTermination {
        let start = Instant::now();
        loop {
            if control.is_cancelled() {
                return ExecTermination::Cancelled;
            }
            if lock(&self.procs).get(&handle.0) == Some(&ProcessStatus::Killed) {
                return ExecTermination::Killed;
            }
            if start.elapsed() >= Duration::from_millis(spec.timeout_ms) {
                return ExecTermination::TimedOut;
            }
            std::thread::sleep(Duration::from_millis(2));
        }
    }
}

impl ProcessPort for FakeExec {
    fn spawn(&self, spec: ProcessSpec) -> Result<ProcessHandle, PlatformError> {
        if spec.cmd.as_os_str().is_empty() {
            return Err(PlatformError::InvalidPath(spec.cmd));
        }
        Ok(self.allocate())
    }

    fn kill_tree(&self, handle: ProcessHandle) -> Result<(), PlatformError> {
        let mut procs = lock(&self.procs);
        let status = procs
            .get_mut(&handle.0)
            .ok_or_else(|| PlatformError::UnknownResource(format!("proces {}", handle.0)))?;
        if *status == ProcessStatus::Running {
            *status = ProcessStatus::Killed;
        }
        Ok(())
    }

    fn status(&self, handle: ProcessHandle) -> Result<ProcessStatus, PlatformError> {
        lock(&self.procs)
            .get(&handle.0)
            .copied()
            .ok_or_else(|| PlatformError::UnknownResource(format!("proces {}", handle.0)))
    }

    fn foreground_is_elevated(&self) -> bool {
        false
    }
}

impl ExecPort for FakeExec {
    fn run_captured(
        &self,
        spec: ExecSpec,
        control: &ExecControl,
    ) -> Result<ExecOutput, PlatformError> {
        spec.validate()?;
        if spec.process.cmd.as_os_str().is_empty() {
            return Err(PlatformError::InvalidPath(spec.process.cmd));
        }
        if let Some(err) = lock(&self.fail_spawn).take() {
            return Err(err);
        }
        lock(&self.runs).push(spec.clone());
        let run = self.pick(&spec);
        let handle = self.allocate();
        control.notify_spawned(handle);
        let (termination, elapsed_ms) = if run.hang {
            let t = self.wait_hanging(handle, &spec, control);
            (t, spec.timeout_ms.min(run.duration_ms.max(1)))
        } else if control.is_cancelled() {
            (ExecTermination::Cancelled, 0)
        } else if run.duration_ms > spec.timeout_ms {
            (ExecTermination::TimedOut, spec.timeout_ms)
        } else {
            (ExecTermination::Exited(run.exit_code), run.duration_ms)
        };
        if termination != ExecTermination::Cancelled
            && let Some(effect) = &run.effect
        {
            effect(&spec);
        }
        let final_status = match termination {
            ExecTermination::Exited(code) => ProcessStatus::Exited(code),
            _ => ProcessStatus::Killed,
        };
        self.set_status(handle, final_status);
        let mut out = CapturedStream::new(spec.max_output_bytes);
        let mut err = CapturedStream::new(spec.max_output_bytes);
        if !run.hang {
            out.push(&run.stdout);
            err.push(&run.stderr);
        }
        let (stdout, stdout_total) = out.finish();
        let (stderr, stderr_total) = err.finish();
        Ok(ExecOutput {
            handle: Some(handle),
            termination,
            stdout,
            stderr,
            stdout_total,
            stderr_total,
            elapsed_ms,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn spec(args: &[&str], timeout_ms: u64) -> ExecSpec {
        ExecSpec {
            process: ProcessSpec {
                cmd: PathBuf::from("C:/pwsh.exe"),
                args: args.iter().map(|a| (*a).to_owned()).collect(),
                cwd: PathBuf::from("C:/w"),
                integrity: Default::default(),
                memory_limit_mb: None,
            },
            raw_args: None,
            env: vec![],
            timeout_ms,
            max_output_bytes: 4,
        }
    }

    #[test]
    fn queue_rules_default_and_limits() {
        let ex = FakeExec::new();
        ex.on_command("dir", FakeRun::ok("listing"));
        ex.push(FakeRun::exit(3, "", "boom"));
        let c = ExecControl::new();
        let first = ex.run_captured(spec(&["dir"], 100), &c).unwrap();
        assert_eq!(first.termination, ExecTermination::Exited(3));
        assert_eq!(first.stderr, b"boom");
        let second = ex.run_captured(spec(&["DIR"], 100), &c).unwrap();
        assert_eq!(second.stdout, b"list");
        assert_eq!(second.stdout_total, 7);
        assert!(second.truncated());
        let third = ex.run_captured(spec(&["other"], 100), &c).unwrap();
        assert_eq!(third.termination, ExecTermination::Exited(0));
        assert_eq!(ex.runs().len(), 3);
        let h = third.handle.unwrap();
        assert_eq!(ex.status(h).unwrap(), ProcessStatus::Exited(0));
    }

    #[test]
    fn timeout_cancel_and_kill() {
        let ex = FakeExec::new();
        ex.push(FakeRun::ok("x").taking(500));
        let out = ex
            .run_captured(spec(&["slow"], 100), &ExecControl::new())
            .unwrap();
        assert_eq!(out.termination, ExecTermination::TimedOut);
        assert_eq!(
            ex.status(out.handle.unwrap()).unwrap(),
            ProcessStatus::Killed
        );

        ex.push(FakeRun::hanging());
        let c = ExecControl::new();
        c.cancel();
        let out = ex.run_captured(spec(&["hang"], 10_000), &c).unwrap();
        assert_eq!(out.termination, ExecTermination::Cancelled);

        let ex = Arc::new(ex);
        ex.push(FakeRun::hanging());
        let ex2 = ex.clone();
        let c = ExecControl::new().on_spawn(move |h| {
            let ex3 = ex2.clone();
            std::thread::spawn(move || {
                std::thread::sleep(Duration::from_millis(5));
                let _ = ex3.kill_tree(h);
            });
        });
        let out = ex.run_captured(spec(&["hang"], 10_000), &c).unwrap();
        assert_eq!(out.termination, ExecTermination::Killed);

        ex.push(FakeRun::hanging());
        let out = ex
            .run_captured(spec(&["hang"], 20), &ExecControl::new())
            .unwrap();
        assert_eq!(out.termination, ExecTermination::TimedOut);
    }

    #[test]
    fn effects_errors_and_process_port() {
        let ex = FakeExec::new();
        let hits = Arc::new(Mutex::new(0));
        let h2 = hits.clone();
        ex.push(FakeRun::ok("").with_effect(move |_| *h2.lock().unwrap() += 1));
        ex.run_captured(spec(&["a"], 100), &ExecControl::new())
            .unwrap();
        assert_eq!(*hits.lock().unwrap(), 1);
        ex.fail_next_spawn(PlatformError::Io("x".into()));
        assert!(
            ex.run_captured(spec(&["a"], 100), &ExecControl::new())
                .is_err()
        );
        assert!(
            ex.run_captured(spec(&["a"], 0), &ExecControl::new())
                .is_err()
        );
        let h = ex.spawn(spec(&[], 1).process).unwrap();
        ex.kill_tree(h).unwrap();
        assert_eq!(ex.status(h).unwrap(), ProcessStatus::Killed);
        assert!(ex.kill_tree(ProcessHandle(999)).is_err());
        assert!(!ex.foreground_is_elevated());
        assert!(format!("{ex:?} {:?}", FakeRun::ok("a")).contains("FakeRun"));
    }
}
