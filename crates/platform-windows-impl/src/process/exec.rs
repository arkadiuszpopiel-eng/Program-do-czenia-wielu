//! `ExecPort` (rozszerzenie `ProcessPort`): uruchomienie w Job Object z potokami stdout/stderr,
//! jawnym środowiskiem (bez dziedziczenia), limitem czasu i wyjścia oraz anulowaniem. Proces
//! trafia do tej samej tabeli co `spawn`, więc `kill_tree` z kill-switcha zabija także jego drzewo.
//! Pętla oczekiwania i czytniki potoków są przenośne (testy na Linux/CI); start procesu — Windows.

use std::io::Read;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use platform_contract::{
    CapturedStream, ExecControl, ExecOutput, ExecPort, ExecSpec, ExecTermination, Integrity,
    PlatformError, ProcessStatus,
};
#[cfg(windows)]
use platform_contract::{ProcessHandle, ProcessPort};

use super::WinProcesses;

/// Co ile sprawdzamy stan procesu i flagę anulowania.
const POLL: Duration = Duration::from_millis(10);
/// Ile czekamy na domknięcie potoków po końcu procesu głównego (potomkowie mogą je trzymać).
const DRAIN_GRACE: Duration = Duration::from_millis(500);
/// Ile czekamy na czytniki po zabiciu drzewa.
const DRAIN_AFTER_KILL: Duration = Duration::from_secs(5);

/// Argumenty w jednym wierszu wg reguł MSVCRT/`CommandLineToArgvW` (bez programu).
#[cfg_attr(not(windows), allow(dead_code))]
pub(crate) fn args_line(args: &[String]) -> String {
    let mut out = String::new();
    for arg in args {
        if !out.is_empty() {
            out.push(' ');
        }
        super::cmdline::push_quoted(&mut out, arg);
    }
    out
}

/// Czytnik potoku na osobnym wątku: zbiera ≤ `limit` bajtów, liczy wszystkie, czyta do EOF
/// (proces nie blokuje się na pełnym potoku).
#[cfg_attr(not(windows), allow(dead_code))]
pub(crate) fn spawn_reader<R: Read + Send + 'static>(
    mut reader: R,
    limit: usize,
    name: &str,
) -> Result<mpsc::Receiver<(Vec<u8>, u64)>, PlatformError> {
    let (tx, rx) = mpsc::channel();
    std::thread::Builder::new()
        .name(name.to_owned())
        .spawn(move || {
            let mut captured = CapturedStream::new(limit);
            let mut buf = [0u8; 8192];
            loop {
                match reader.read(&mut buf) {
                    Ok(0) => break,
                    Ok(n) => captured.push(&buf[..n]),
                    Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
                    Err(_) => break,
                }
            }
            let _ = tx.send(captured.finish());
        })
        .map_err(|e| PlatformError::Io(format!("wątek czytnika potoku: {e}")))?;
    Ok(rx)
}

/// Czeka na koniec procesu: sam (kod), zabity z zewnątrz, anulowany albo po limicie czasu
/// (wtedy zabija drzewo przez `kill`).
#[cfg_attr(not(windows), allow(dead_code))]
pub(crate) fn wait_for_exit(
    status: impl Fn() -> Result<ProcessStatus, PlatformError>,
    kill: impl Fn() -> Result<(), PlatformError>,
    control: &ExecControl,
    timeout: Duration,
    started: Instant,
) -> Result<ExecTermination, PlatformError> {
    loop {
        match status()? {
            ProcessStatus::Exited(code) => return Ok(ExecTermination::Exited(code)),
            ProcessStatus::Killed => return Ok(ExecTermination::Killed),
            ProcessStatus::Running => {}
        }
        if control.is_cancelled() {
            kill()?;
            return Ok(ExecTermination::Cancelled);
        }
        if started.elapsed() >= timeout {
            kill()?;
            return Ok(ExecTermination::TimedOut);
        }
        std::thread::sleep(POLL);
    }
}

/// Odbiera wyniki czytników; gdy potomkowie trzymają potoki dłużej niż `DRAIN_GRACE`, zabija
/// drzewo (koniec polecenia = koniec jego procesów) i czeka jeszcze `DRAIN_AFTER_KILL`
/// (`waits` = (`DRAIN_GRACE`, `DRAIN_AFTER_KILL`) w produkcji).
#[cfg_attr(not(windows), allow(dead_code))]
pub(crate) fn drain(
    readers: [&mpsc::Receiver<(Vec<u8>, u64)>; 2],
    kill: impl Fn() -> Result<(), PlatformError>,
    waits: (Duration, Duration),
) -> [(Vec<u8>, u64); 2] {
    let (grace, after_kill) = waits;
    let mut results: [Option<(Vec<u8>, u64)>; 2] = [None, None];
    for (slot, rx) in results.iter_mut().zip(readers) {
        *slot = rx.recv_timeout(grace).ok();
    }
    if results.iter().any(Option::is_none) {
        let _ = kill();
        for (slot, rx) in results.iter_mut().zip(readers) {
            if slot.is_none() {
                *slot = rx.recv_timeout(after_kill).ok();
            }
        }
    }
    results.map(Option::unwrap_or_default)
}

/// Wspólne sprawdzenia przed startem (każda platforma).
fn precheck(spec: &ExecSpec) -> Result<(), PlatformError> {
    spec.validate()?;
    super::cmdline::validate_spec(&spec.process)?;
    if spec.process.integrity != Integrity::Medium {
        // Proces o niskiej integralności nie zapisze plików użytkownika (zakres shella);
        // AppContainer dochodzi z izolacją narzędzi (F3, część 2 Brokera).
        return Err(PlatformError::Unsupported(
            "uruchamianie z przechwyceniem: tylko integralność Medium w Job Object".into(),
        ));
    }
    Ok(())
}

impl ExecPort for WinProcesses {
    fn run_captured(
        &self,
        spec: ExecSpec,
        control: &ExecControl,
    ) -> Result<ExecOutput, PlatformError> {
        precheck(&spec)?;
        #[cfg(windows)]
        {
            let limits = self.defaults.merge(super::JobLimits {
                memory_mb: spec.process.memory_limit_mb,
                ..super::JobLimits::default()
            });
            let started = Instant::now();
            let spawned = super::win::exec::spawn_captured(&spec, &limits)?;
            let handle = ProcessHandle(spawned.child.pid());
            self.lock().insert(handle.0, spawned.child);
            control.notify_spawned(handle);
            let readers = [
                spawn_reader(spawned.stdout, spec.max_output_bytes, "alfa-exec-stdout"),
                spawn_reader(spawned.stderr, spec.max_output_bytes, "alfa-exec-stderr"),
            ];
            let (out_rx, err_rx) = match readers {
                [Ok(o), Ok(e)] => (o, e),
                [Err(e), _] | [_, Err(e)] => {
                    let _ = self.kill_tree(handle);
                    let _ = self.release(handle);
                    return Err(e);
                }
            };
            let timeout = Duration::from_millis(spec.timeout_ms);
            let termination = wait_for_exit(
                || self.status(handle),
                || self.kill_tree(handle),
                control,
                timeout,
                started,
            );
            let [(stdout, stdout_total), (stderr, stderr_total)] = drain(
                [&out_rx, &err_rx],
                || self.kill_tree(handle),
                (DRAIN_GRACE, DRAIN_AFTER_KILL),
            );
            let _ = self.release(handle);
            Ok(ExecOutput {
                handle: Some(handle),
                termination: termination?,
                stdout,
                stderr,
                stdout_total,
                stderr_total,
                elapsed_ms: u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
            })
        }
        #[cfg(not(windows))]
        {
            let _ = control;
            Err(PlatformError::Unsupported(
                "uruchamianie procesów w Job Object tylko na Windows".into(),
            ))
        }
    }
}

impl ExecPort for crate::WindowsPlatform {
    fn run_captured(
        &self,
        spec: ExecSpec,
        control: &ExecControl,
    ) -> Result<ExecOutput, PlatformError> {
        self.processes.run_captured(spec, control)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicU32, Ordering};

    use platform_contract::ProcessSpec;

    use crate::process::JobLimits;

    fn spec(cmd: &str) -> ExecSpec {
        ExecSpec {
            process: ProcessSpec {
                cmd: PathBuf::from(cmd),
                args: vec![],
                cwd: std::env::temp_dir(),
                integrity: Integrity::Medium,
                memory_limit_mb: None,
            },
            raw_args: None,
            env: vec![],
            timeout_ms: 1000,
            max_output_bytes: 16,
        }
    }

    #[test]
    fn args_line_quotes_like_msvcrt() {
        let args = vec!["-NoProfile".to_owned(), "a b".to_owned(), "q\"x".to_owned()];
        assert_eq!(args_line(&args), r#"-NoProfile "a b" "q\"x""#);
        assert_eq!(args_line(&[]), "");
    }

    #[test]
    fn reader_limits_and_counts() {
        let rx = spawn_reader(std::io::Cursor::new(vec![7u8; 100]), 10, "t").unwrap();
        let (data, total) = rx.recv_timeout(Duration::from_secs(5)).unwrap();
        assert_eq!((data.len(), total), (10, 100));
    }

    #[test]
    fn wait_loop_exit_kill_cancel_timeout() {
        let polls = AtomicU32::new(0);
        let status = || {
            let n = polls.fetch_add(1, Ordering::SeqCst);
            Ok(if n < 2 {
                ProcessStatus::Running
            } else {
                ProcessStatus::Exited(4)
            })
        };
        let c = ExecControl::new();
        let t = wait_for_exit(
            status,
            || Ok(()),
            &c,
            Duration::from_secs(5),
            Instant::now(),
        );
        assert_eq!(t.unwrap(), ExecTermination::Exited(4));

        let killed = || Ok(ProcessStatus::Killed);
        let t = wait_for_exit(
            killed,
            || Ok(()),
            &c,
            Duration::from_secs(5),
            Instant::now(),
        );
        assert_eq!(t.unwrap(), ExecTermination::Killed);

        let kills = Mutex::new(0);
        let running = || Ok(ProcessStatus::Running);
        let kill = || {
            *kills.lock().unwrap() += 1;
            Ok(())
        };
        let t = wait_for_exit(running, kill, &c, Duration::ZERO, Instant::now());
        assert_eq!(t.unwrap(), ExecTermination::TimedOut);
        c.cancel();
        let t = wait_for_exit(running, kill, &c, Duration::from_secs(5), Instant::now());
        assert_eq!(t.unwrap(), ExecTermination::Cancelled);
        assert_eq!(*kills.lock().unwrap(), 2);
        let failing = || Err(PlatformError::Io("x".into()));
        assert!(wait_for_exit(failing, kill, &c, Duration::ZERO, Instant::now()).is_err());
    }

    #[test]
    fn drain_kills_when_pipes_stay_open() {
        let (tx_out, rx_out) = mpsc::channel();
        let (_keep_err_open, rx_err) = mpsc::channel::<(Vec<u8>, u64)>();
        tx_out.send((b"ok".to_vec(), 2)).unwrap();
        let kills = Mutex::new(0);
        let started = Instant::now();
        let waits = (Duration::from_millis(20), Duration::from_millis(20));
        let [(out, n), (err, m)] = drain(
            [&rx_out, &rx_err],
            || {
                *kills.lock().unwrap() += 1;
                Ok(())
            },
            waits,
        );
        assert_eq!((out, n, err, m), (b"ok".to_vec(), 2, vec![], 0));
        assert_eq!(*kills.lock().unwrap(), 1);
        assert!(started.elapsed() >= Duration::from_millis(40));
        assert!(DRAIN_GRACE < DRAIN_AFTER_KILL);
    }

    #[test]
    fn precheck_rejects_bad_specs() {
        let p = WinProcesses::new(JobLimits::default());
        let c = ExecControl::new();
        assert!(matches!(
            p.run_captured(spec("relative.exe"), &c),
            Err(PlatformError::InvalidPath(_))
        ));
        let mut low = spec(if cfg!(windows) {
            r"C:\Windows\System32\cmd.exe"
        } else {
            "/bin/sh"
        });
        low.process.integrity = Integrity::Low;
        assert!(matches!(
            p.run_captured(low, &c),
            Err(PlatformError::Unsupported(_))
        ));
        let mut zero = spec("/bin/sh");
        zero.timeout_ms = 0;
        assert!(p.run_captured(zero, &c).is_err());
        if !cfg!(windows) {
            assert!(matches!(
                p.run_captured(spec("/bin/sh"), &c),
                Err(PlatformError::Unsupported(_))
            ));
        }
    }
}
