//! Win32 dla procesów: `CreateProcessW` (wstrzymany) → `AssignProcessToJobObject` → `ResumeThread`,
//! token niskiej integralności, `TerminateJobObject`, Toolhelp32, `TokenElevation`.

#![allow(unsafe_code)]

pub(crate) mod exec;

use std::ffi::c_void;
use std::mem::size_of;

use platform_contract::{Integrity, PlatformError, ProcessSpec, ProcessStatus};
use windows::Win32::Foundation::{HANDLE, HLOCAL, LocalFree, WAIT_OBJECT_0};
use windows::Win32::Security::Authorization::ConvertStringSidToSidW;
use windows::Win32::Security::{
    DuplicateTokenEx, GetLengthSid, GetTokenInformation, PSID, SID_AND_ATTRIBUTES,
    SecurityImpersonation, SetTokenInformation, TOKEN_ACCESS_MASK, TOKEN_ADJUST_DEFAULT,
    TOKEN_ASSIGN_PRIMARY, TOKEN_DUPLICATE, TOKEN_ELEVATION, TOKEN_MANDATORY_LABEL, TOKEN_QUERY,
    TokenElevation, TokenIntegrityLevel, TokenPrimary,
};
use windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW, TH32CS_SNAPPROCESS,
};
use windows::Win32::System::JobObjects::{
    AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_CPU_RATE_CONTROL_ENABLE,
    JOB_OBJECT_CPU_RATE_CONTROL_HARD_CAP, JOB_OBJECT_LIMIT_AFFINITY,
    JOB_OBJECT_LIMIT_DIE_ON_UNHANDLED_EXCEPTION, JOB_OBJECT_LIMIT_JOB_MEMORY,
    JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE, JOBOBJECT_BASIC_ACCOUNTING_INFORMATION,
    JOBOBJECT_CPU_RATE_CONTROL_INFORMATION, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
    JobObjectBasicAccountingInformation, JobObjectCpuRateControlInformation,
    JobObjectExtendedLimitInformation, QueryInformationJobObject, SetInformationJobObject,
    TerminateJobObject,
};
use windows::Win32::System::Threading::{
    CREATE_NO_WINDOW, CREATE_SUSPENDED, CREATE_UNICODE_ENVIRONMENT, CreateProcessAsUserW,
    CreateProcessW, GetCurrentProcess, GetExitCodeProcess, OpenProcess, OpenProcessToken,
    PROCESS_INFORMATION, PROCESS_QUERY_LIMITED_INFORMATION, ResumeThread, STARTUPINFOW,
    TerminateProcess, WaitForSingleObject,
};
use windows::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowThreadProcessId};
use windows::core::{PWSTR, w};

use super::{JobLimits, ProcessInfo};
use crate::win::{OwnedHandle, from_wide, last_error, pcwstr, wide, win_error};

/// Kod wyjścia procesów zabitych przez `kill_tree`.
const KILLED_EXIT_CODE: u32 = 0xA1FA_0001;
/// Ile czekamy na potwierdzenie śmierci głównego procesu po `TerminateJobObject`.
const KILL_WAIT_MS: u32 = 150;
/// `SE_GROUP_INTEGRITY` (winnt.h).
const SE_GROUP_INTEGRITY: u32 = 0x20;

/// Proces uruchomiony w Job Object.
#[derive(Debug)]
pub(crate) struct Child {
    pid: u32,
    job: OwnedHandle,
    process: OwnedHandle,
    killed: bool,
}

fn job_error(context: &str) -> impl Fn(windows::core::Error) -> PlatformError + '_ {
    move |e| win_error(context, &e)
}

fn create_job(limits: &JobLimits) -> Result<OwnedHandle, PlatformError> {
    // SAFETY: anonimowy Job Object bez atrybutów bezpieczeństwa.
    let raw = unsafe { CreateJobObjectW(None, None) }.map_err(job_error("CreateJobObjectW"))?;
    let job = OwnedHandle::new(raw).ok_or_else(|| last_error("CreateJobObjectW"))?;
    let mut info = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
    let mut flags =
        JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE | JOB_OBJECT_LIMIT_DIE_ON_UNHANDLED_EXCEPTION;
    if let Some(mb) = limits.memory_mb {
        flags |= JOB_OBJECT_LIMIT_JOB_MEMORY;
        info.JobMemoryLimit = usize::try_from(u64::from(mb) * 1024 * 1024).unwrap_or(usize::MAX);
    }
    if let Some(mask) = limits.affinity_mask {
        flags |= JOB_OBJECT_LIMIT_AFFINITY;
        info.BasicLimitInformation.Affinity = usize::try_from(mask).unwrap_or(usize::MAX);
    }
    info.BasicLimitInformation.LimitFlags = flags;
    // SAFETY: `info` żyje przez wywołanie, rozmiar odpowiada klasie informacji.
    unsafe {
        SetInformationJobObject(
            job.raw(),
            JobObjectExtendedLimitInformation,
            (&raw const info).cast::<c_void>(),
            u32::try_from(size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>()).unwrap_or(0),
        )
    }
    .map_err(job_error("SetInformationJobObject(limity)"))?;
    if let Some(pct) = limits.cpu_rate_percent {
        let mut cpu = JOBOBJECT_CPU_RATE_CONTROL_INFORMATION {
            ControlFlags: JOB_OBJECT_CPU_RATE_CONTROL_ENABLE | JOB_OBJECT_CPU_RATE_CONTROL_HARD_CAP,
            ..Default::default()
        };
        // Jednostka: 1/100 procenta (10 000 = 100%).
        cpu.Anonymous.CpuRate = u32::from(pct.clamp(1, 100)) * 100;
        // SAFETY: jw. dla struktury limitu CPU.
        unsafe {
            SetInformationJobObject(
                job.raw(),
                JobObjectCpuRateControlInformation,
                (&raw const cpu).cast::<c_void>(),
                u32::try_from(size_of::<JOBOBJECT_CPU_RATE_CONTROL_INFORMATION>()).unwrap_or(0),
            )
        }
        .map_err(job_error("SetInformationJobObject(CPU)"))?;
    }
    Ok(job)
}

/// Kopia tokenu procesu z poziomem integralności Low (S-1-16-4096).
fn low_integrity_token() -> Result<OwnedHandle, PlatformError> {
    let mut own = HANDLE::default();
    let access = TOKEN_DUPLICATE | TOKEN_QUERY | TOKEN_ADJUST_DEFAULT | TOKEN_ASSIGN_PRIMARY;
    // SAFETY: pseudo-uchwyt bieżącego procesu; wynik trafia do `own`.
    unsafe { OpenProcessToken(GetCurrentProcess(), access, &raw mut own) }
        .map_err(job_error("OpenProcessToken"))?;
    let own = OwnedHandle::new(own).ok_or_else(|| last_error("OpenProcessToken"))?;
    let mut dup = HANDLE::default();
    // SAFETY: `own` to ważny token; dostęp 0 = taki sam jak źródła.
    unsafe {
        DuplicateTokenEx(
            own.raw(),
            TOKEN_ACCESS_MASK(0),
            None,
            SecurityImpersonation,
            TokenPrimary,
            &raw mut dup,
        )
    }
    .map_err(job_error("DuplicateTokenEx"))?;
    let token = OwnedHandle::new(dup).ok_or_else(|| last_error("DuplicateTokenEx"))?;
    let mut sid = PSID::default();
    // SAFETY: literał SID zakończony zerem; SID zwalniamy `LocalFree` niżej.
    unsafe { ConvertStringSidToSidW(w!("S-1-16-4096"), &raw mut sid) }
        .map_err(job_error("ConvertStringSidToSidW"))?;
    let label = TOKEN_MANDATORY_LABEL {
        Label: SID_AND_ATTRIBUTES {
            Sid: sid,
            Attributes: SE_GROUP_INTEGRITY,
        },
    };
    // SAFETY: `sid` jest ważny (z `ConvertStringSidToSidW`).
    let sid_len = unsafe { GetLengthSid(sid) };
    let size = u32::try_from(size_of::<TOKEN_MANDATORY_LABEL>()).unwrap_or(0) + sid_len;
    // SAFETY: `label` wskazuje na ważny SID; rozmiar = struktura + SID.
    let set = unsafe {
        SetTokenInformation(
            token.raw(),
            TokenIntegrityLevel,
            (&raw const label).cast::<c_void>(),
            size,
        )
    };
    // SAFETY: SID zaalokowany przez `ConvertStringSidToSidW`, zwalniany dokładnie raz.
    unsafe { LocalFree(Some(HLOCAL(sid.0))) };
    set.map_err(job_error("SetTokenInformation(TokenIntegrityLevel)"))?;
    Ok(token)
}

/// Uruchamia proces wstrzymany, przypisuje do nowego Job Object i wznawia.
pub(crate) fn spawn(
    spec: &ProcessSpec,
    command_line: &str,
    limits: &JobLimits,
) -> Result<Child, PlatformError> {
    let token = match spec.integrity {
        Integrity::Medium => None,
        Integrity::Low => Some(low_integrity_token()?),
        Integrity::AppContainer => {
            return Err(PlatformError::Unsupported(
                "AppContainer dochodzi z Brokerem (F3)".into(),
            ));
        }
    };
    let job = create_job(limits)?;
    let app = wide(&spec.cmd);
    let cwd = wide(&spec.cwd);
    let mut line = wide(command_line);
    let startup = STARTUPINFOW {
        cb: u32::try_from(size_of::<STARTUPINFOW>()).unwrap_or(0),
        ..Default::default()
    };
    let mut info = PROCESS_INFORMATION::default();
    let flags = CREATE_SUSPENDED | CREATE_UNICODE_ENVIRONMENT | CREATE_NO_WINDOW;
    let created = match &token {
        // SAFETY: bufory zakończone zerem żyją przez wywołanie; `line` jest modyfikowalny (wymóg API);
        // uchwyty dziedziczone wyłączone.
        None => unsafe {
            CreateProcessW(
                pcwstr(&app),
                Some(PWSTR(line.as_mut_ptr())),
                None,
                None,
                false,
                flags,
                None,
                pcwstr(&cwd),
                &raw const startup,
                &raw mut info,
            )
        },
        // SAFETY: jw.; token to kopia tokenu bieżącego procesu z obniżoną integralnością.
        Some(token) => unsafe {
            CreateProcessAsUserW(
                Some(token.raw()),
                pcwstr(&app),
                Some(PWSTR(line.as_mut_ptr())),
                None,
                None,
                false,
                flags,
                None,
                pcwstr(&cwd),
                &raw const startup,
                &raw mut info,
            )
        },
    };
    created.map_err(|e| win_error(&format!("CreateProcess({})", spec.cmd.display()), &e))?;
    let process = OwnedHandle::new(info.hProcess).ok_or_else(|| last_error("CreateProcess"))?;
    let thread = OwnedHandle::new(info.hThread).ok_or_else(|| last_error("CreateProcess"))?;
    // SAFETY: oba uchwyty są ważne; proces jest wstrzymany, więc nie zdążył utworzyć potomków.
    if let Err(e) = unsafe { AssignProcessToJobObject(job.raw(), process.raw()) } {
        // SAFETY: zabijamy własny, wstrzymany proces, którego nie udało się objąć limitami.
        let _ = unsafe { TerminateProcess(process.raw(), KILLED_EXIT_CODE) };
        return Err(win_error("AssignProcessToJobObject", &e));
    }
    // SAFETY: `thread` to główny wątek wstrzymanego procesu.
    if unsafe { ResumeThread(thread.raw()) } == u32::MAX {
        let err = last_error("ResumeThread");
        // SAFETY: jw. — proces nie ruszył, sprzątamy go.
        let _ = unsafe { TerminateJobObject(job.raw(), KILLED_EXIT_CODE) };
        return Err(err);
    }
    Ok(Child {
        pid: info.dwProcessId,
        job,
        process,
        killed: false,
    })
}

impl Child {
    /// PID procesu głównego.
    pub(crate) fn pid(&self) -> u32 {
        self.pid
    }

    /// Zabija całe drzewo (wszystkie procesy w Job Object) i czeka krótko na proces główny.
    pub(crate) fn kill_tree(&mut self) -> Result<(), PlatformError> {
        // SAFETY: `job` to ważny uchwyt z prawem JOB_OBJECT_TERMINATE (utworzyliśmy go sami).
        unsafe { TerminateJobObject(self.job.raw(), KILLED_EXIT_CODE) }
            .map_err(job_error("TerminateJobObject"))?;
        self.killed = true;
        // SAFETY: ważny uchwyt procesu; oczekiwanie ograniczone czasowo.
        let _ = unsafe { WaitForSingleObject(self.process.raw(), KILL_WAIT_MS) };
        Ok(())
    }

    /// Stan procesu głównego.
    pub(crate) fn status(&self) -> Result<ProcessStatus, PlatformError> {
        if self.killed {
            return Ok(ProcessStatus::Killed);
        }
        // SAFETY: ważny uchwyt procesu; zerowy czas oczekiwania.
        if unsafe { WaitForSingleObject(self.process.raw(), 0) } != WAIT_OBJECT_0 {
            return Ok(ProcessStatus::Running);
        }
        let mut code = 0u32;
        // SAFETY: proces zakończony, `code` to poprawny bufor wyjściowy.
        unsafe { GetExitCodeProcess(self.process.raw(), &raw mut code) }
            .map_err(job_error("GetExitCodeProcess"))?;
        Ok(ProcessStatus::Exited(i32::from_ne_bytes(
            code.to_ne_bytes(),
        )))
    }

    /// Liczba aktywnych procesów w Job Object.
    pub(crate) fn active_processes(&self) -> Result<u32, PlatformError> {
        let mut info = JOBOBJECT_BASIC_ACCOUNTING_INFORMATION::default();
        // SAFETY: bufor wyjściowy o rozmiarze klasy informacji.
        unsafe {
            QueryInformationJobObject(
                Some(self.job.raw()),
                JobObjectBasicAccountingInformation,
                (&raw mut info).cast::<c_void>(),
                u32::try_from(size_of::<JOBOBJECT_BASIC_ACCOUNTING_INFORMATION>()).unwrap_or(0),
                None,
            )
        }
        .map_err(job_error("QueryInformationJobObject"))?;
        Ok(info.ActiveProcesses)
    }
}

/// Wszystkie procesy systemu (Toolhelp32).
pub(crate) fn list_processes() -> Result<Vec<ProcessInfo>, PlatformError> {
    // SAFETY: migawka procesów, uchwyt przejmuje `OwnedHandle`.
    let raw = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) }
        .map_err(job_error("CreateToolhelp32Snapshot"))?;
    let snapshot = OwnedHandle::new(raw).ok_or_else(|| last_error("CreateToolhelp32Snapshot"))?;
    let mut entry = PROCESSENTRY32W {
        dwSize: u32::try_from(size_of::<PROCESSENTRY32W>()).unwrap_or(0),
        ..Default::default()
    };
    let mut out = Vec::new();
    // SAFETY: `entry.dwSize` ustawione zgodnie z wymogiem API; migawka ważna.
    let mut more = unsafe { Process32FirstW(snapshot.raw(), &raw mut entry) }.is_ok();
    while more {
        out.push(ProcessInfo {
            pid: entry.th32ProcessID,
            parent_pid: entry.th32ParentProcessID,
            name: from_wide(&entry.szExeFile),
        });
        // SAFETY: jw.
        more = unsafe { Process32NextW(snapshot.raw(), &raw mut entry) }.is_ok();
    }
    Ok(out)
}

/// Czy okno na pierwszym planie należy do procesu podniesionego (hooki i PTT wtedy nie działają).
/// Brak dostępu do tokenu procesu okna traktujemy jako „podniesiony” (UIPI).
pub(crate) fn foreground_is_elevated() -> bool {
    // SAFETY: odczyt uchwytu okna pierwszego planu (może być pusty).
    let hwnd = unsafe { GetForegroundWindow() };
    if hwnd.is_invalid() {
        return false;
    }
    let mut pid = 0u32;
    // SAFETY: `pid` to poprawny bufor wyjściowy.
    unsafe { GetWindowThreadProcessId(hwnd, Some(&raw mut pid)) };
    if pid == 0 {
        return false;
    }
    // SAFETY: minimalne prawo zapytania, uchwyt przejmuje `OwnedHandle`.
    let Ok(process) = (unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) })
    else {
        return true;
    };
    let Some(process) = OwnedHandle::new(process) else {
        return true;
    };
    let mut token = HANDLE::default();
    // SAFETY: ważny uchwyt procesu; token trafia do `token`.
    if unsafe { OpenProcessToken(process.raw(), TOKEN_QUERY, &raw mut token) }.is_err() {
        return true;
    }
    let Some(token) = OwnedHandle::new(token) else {
        return true;
    };
    let mut elevation = TOKEN_ELEVATION::default();
    let mut returned = 0u32;
    // SAFETY: bufor wyjściowy o rozmiarze `TOKEN_ELEVATION`.
    let ok = unsafe {
        GetTokenInformation(
            token.raw(),
            TokenElevation,
            Some((&raw mut elevation).cast::<c_void>()),
            u32::try_from(size_of::<TOKEN_ELEVATION>()).unwrap_or(0),
            &raw mut returned,
        )
    };
    ok.is_err() || elevation.TokenIsElevated != 0
}
