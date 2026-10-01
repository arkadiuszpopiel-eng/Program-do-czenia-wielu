//! Win32: start procesu z potokami stdout/stderr. `std::process::Command` (potoki, środowisko
//! wyłącznie jawne — `env_clear`, cudzysłowy przez `raw_arg`) w stanie `CREATE_SUSPENDED` →
//! `AssignProcessToJobObject` → wznowienie wątku głównego (Toolhelp32). Proces nie wykona żadnej
//! instrukcji, zanim nie znajdzie się w Job Object (limity, zabijanie całego drzewa).

#![allow(unsafe_code)]

use std::os::windows::io::IntoRawHandle;
use std::os::windows::process::CommandExt;
use std::process::{ChildStderr, ChildStdout, Command, Stdio};

use platform_contract::{ExecSpec, PlatformError};
use windows::Win32::Foundation::HANDLE;
use windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, TH32CS_SNAPTHREAD, THREADENTRY32, Thread32First, Thread32Next,
};
use windows::Win32::System::JobObjects::{AssignProcessToJobObject, TerminateJobObject};
use windows::Win32::System::Threading::{
    CREATE_NO_WINDOW, CREATE_SUSPENDED, OpenThread, ResumeThread, THREAD_SUSPEND_RESUME,
    TerminateProcess,
};

use super::{Child, KILLED_EXIT_CODE, create_job, job_error};
use crate::process::JobLimits;
use crate::process::exec::args_line;
use crate::win::{OwnedHandle, last_error, win_error};

/// Proces w Job Object z końcami potoków do odczytu.
pub(crate) struct Spawned {
    /// Proces (do tabeli `WinProcesses`).
    pub(crate) child: Child,
    /// Koniec odczytu stdout.
    pub(crate) stdout: ChildStdout,
    /// Koniec odczytu stderr.
    pub(crate) stderr: ChildStderr,
}

/// Wznawia wątki procesu (nowy proces wstrzymany ma dokładnie jeden — główny).
fn resume_threads(pid: u32) -> Result<(), PlatformError> {
    // SAFETY: migawka wątków systemu, uchwyt przejmuje `OwnedHandle`.
    let raw = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0) }
        .map_err(job_error("CreateToolhelp32Snapshot(wątki)"))?;
    let snapshot = OwnedHandle::new(raw).ok_or_else(|| last_error("CreateToolhelp32Snapshot"))?;
    let mut entry = THREADENTRY32 {
        dwSize: u32::try_from(size_of::<THREADENTRY32>()).unwrap_or(0),
        ..Default::default()
    };
    let mut resumed = 0u32;
    // SAFETY: `entry.dwSize` ustawione zgodnie z wymogiem API; migawka ważna.
    let mut more = unsafe { Thread32First(snapshot.raw(), &raw mut entry) }.is_ok();
    while more {
        if entry.th32OwnerProcessID == pid {
            // SAFETY: minimalne prawo wznowienia; uchwyt przejmuje `OwnedHandle`.
            let thread = unsafe { OpenThread(THREAD_SUSPEND_RESUME, false, entry.th32ThreadID) }
                .map_err(job_error("OpenThread"))?;
            let thread = OwnedHandle::new(thread).ok_or_else(|| last_error("OpenThread"))?;
            // SAFETY: ważny uchwyt wątku naszego wstrzymanego procesu.
            if unsafe { ResumeThread(thread.raw()) } == u32::MAX {
                return Err(last_error("ResumeThread"));
            }
            resumed += 1;
        }
        // SAFETY: jw.
        more = unsafe { Thread32Next(snapshot.raw(), &raw mut entry) }.is_ok();
    }
    if resumed == 0 {
        return Err(PlatformError::Io(format!(
            "proces {pid}: brak wątku do wznowienia"
        )));
    }
    Ok(())
}

/// Uruchamia proces wstrzymany z potokami, obejmuje go Job Object i wznawia.
pub(crate) fn spawn_captured(
    spec: &ExecSpec,
    limits: &JobLimits,
) -> Result<Spawned, PlatformError> {
    let job = create_job(limits)?;
    let mut cmd = Command::new(&spec.process.cmd);
    let args = spec
        .raw_args
        .clone()
        .unwrap_or_else(|| args_line(&spec.process.args));
    if !args.is_empty() {
        cmd.raw_arg(&args);
    }
    cmd.current_dir(&spec.process.cwd)
        .env_clear()
        .envs(spec.env.iter().map(|(k, v)| (k.as_str(), v.as_str())))
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .creation_flags((CREATE_SUSPENDED | CREATE_NO_WINDOW).0);
    let mut child = cmd
        .spawn()
        .map_err(|e| PlatformError::Io(format!("{}: {e}", spec.process.cmd.display())))?;
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    let pid = child.id();
    let process = OwnedHandle::new(HANDLE(child.into_raw_handle()))
        .ok_or_else(|| last_error("CreateProcess"))?;
    // SAFETY: oba uchwyty są ważne; proces jest wstrzymany, więc nie zdążył utworzyć potomków.
    if let Err(e) = unsafe { AssignProcessToJobObject(job.raw(), process.raw()) } {
        // SAFETY: zabijamy własny, wstrzymany proces, którego nie udało się objąć limitami.
        let _ = unsafe { TerminateProcess(process.raw(), KILLED_EXIT_CODE) };
        return Err(win_error("AssignProcessToJobObject", &e));
    }
    let (Some(stdout), Some(stderr)) = (stdout, stderr) else {
        // SAFETY: proces w naszym Job Object, jeszcze wstrzymany — sprzątamy całe drzewo.
        let _ = unsafe { TerminateJobObject(job.raw(), KILLED_EXIT_CODE) };
        return Err(PlatformError::Io("brak potoków stdout/stderr".into()));
    };
    if let Err(e) = resume_threads(pid) {
        // SAFETY: jw. — proces nie ruszył, sprzątamy go.
        let _ = unsafe { TerminateJobObject(job.raw(), KILLED_EXIT_CODE) };
        return Err(e);
    }
    Ok(Spawned {
        child: Child {
            pid,
            job,
            process,
            killed: false,
        },
        stdout,
        stderr,
    })
}
