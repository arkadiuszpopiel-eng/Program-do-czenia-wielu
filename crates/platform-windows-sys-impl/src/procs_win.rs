//! Procesy (tylko Windows): Toolhelp32 (lista, rodzic, wątki), `OpenProcess` z prawem
//! ograniczonego zapytania (ścieżka, czas startu, pamięć, podniesienie, właściciel przez SID tokenu,
//! sesja) i zakończenie przez **ten sam** uchwyt po ponownym sprawdzeniu tożsamości, strażnika
//! celów (pełna ścieżka i świeże drzewo procesów) i właściciela — bez wyścigu z ponownym użyciem
//! PID-u.

#![allow(unsafe_code)]

use std::ffi::c_void;
use std::mem::size_of;

use platform_apps_contract::{
    ProcessDetails, ProcessEntry, ProcessIdentity, SysError, protected_process,
};
use platform_contract::TargetGuard;
use windows::Win32::Foundation::{E_ACCESSDENIED, ERROR_INVALID_PARAMETER, FILETIME, HANDLE};
use windows::Win32::Security::{
    GetLengthSid, GetTokenInformation, TOKEN_ELEVATION, TOKEN_QUERY, TOKEN_USER, TokenElevation,
    TokenUser,
};
use windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW, TH32CS_SNAPPROCESS,
};
use windows::Win32::System::ProcessStatus::{GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS};
use windows::Win32::System::RemoteDesktop::ProcessIdToSessionId;
use windows::Win32::System::Threading::{
    GetCurrentProcess, GetProcessTimes, OpenProcess, OpenProcessToken, PROCESS_ACCESS_RIGHTS,
    PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_TERMINATE,
    QueryFullProcessImageNameW, TerminateProcess,
};
use windows::core::{HRESULT, PWSTR};

use crate::win::{OwnedHandle, from_wide};

/// Kod wyjścia procesu zakończonego przez agentkę.
const EXIT_CODE: u32 = 1;
/// Różnica epok FILETIME (1601) i Unix (1970) w ms.
const EPOCH_DIFF_MS: u64 = 11_644_473_600_000;

/// Błąd windows-rs → `SysError`.
pub(crate) fn sys_error(context: &str, err: &windows::core::Error) -> SysError {
    if err.code() == E_ACCESSDENIED {
        SysError::PermissionDenied(format!("{context}: odmowa dostępu"))
    } else if err.code() == HRESULT::from_win32(ERROR_INVALID_PARAMETER.0) {
        SysError::NotFound(context.to_owned())
    } else {
        SysError::Io(format!("{context}: {err}"))
    }
}

fn open(pid: u32, access: PROCESS_ACCESS_RIGHTS) -> Result<OwnedHandle, SysError> {
    // SAFETY: otwarcie procesu po PID; uchwyt przejmuje `OwnedHandle`.
    let h = unsafe { OpenProcess(access, false, pid) }
        .map_err(|e| sys_error(&format!("proces {pid}"), &e))?;
    OwnedHandle::new(h).ok_or_else(|| SysError::NotFound(format!("proces {pid}")))
}

fn image_path(h: HANDLE) -> Option<String> {
    let mut buf = vec![0u16; 32_768];
    let mut len = u32::try_from(buf.len()).unwrap_or(u32::MAX);
    // SAFETY: bufor wyjściowy o długości `len` znaków.
    unsafe {
        QueryFullProcessImageNameW(h, PROCESS_NAME_WIN32, PWSTR(buf.as_mut_ptr()), &raw mut len)
    }
    .ok()?;
    Some(String::from_utf16_lossy(
        &buf[..(len as usize).min(buf.len())],
    ))
}

fn started_ms(h: HANDLE) -> Option<u64> {
    let (mut c, mut e, mut k, mut u) = (
        FILETIME::default(),
        FILETIME::default(),
        FILETIME::default(),
        FILETIME::default(),
    );
    // SAFETY: cztery struktury wyjściowe.
    unsafe { GetProcessTimes(h, &raw mut c, &raw mut e, &raw mut k, &raw mut u) }.ok()?;
    let ticks = (u64::from(c.dwHighDateTime) << 32) | u64::from(c.dwLowDateTime);
    (ticks / 10_000).checked_sub(EPOCH_DIFF_MS)
}

fn memory_kb(h: HANDLE) -> Option<u64> {
    let mut m = PROCESS_MEMORY_COUNTERS {
        cb: u32::try_from(size_of::<PROCESS_MEMORY_COUNTERS>()).unwrap_or(0),
        ..Default::default()
    };
    // SAFETY: struktura wyjściowa z ustawionym `cb`.
    unsafe { GetProcessMemoryInfo(h, &raw mut m, m.cb) }.ok()?;
    Some((m.WorkingSetSize / 1024) as u64)
}

fn token(h: HANDLE) -> Option<OwnedHandle> {
    let mut t = HANDLE::default();
    // SAFETY: ważny uchwyt procesu; token do `t`.
    unsafe { OpenProcessToken(h, TOKEN_QUERY, &raw mut t) }.ok()?;
    OwnedHandle::new(t)
}

fn elevated(h: HANDLE) -> Option<bool> {
    let t = token(h)?;
    let mut e = TOKEN_ELEVATION::default();
    let mut ret = 0u32;
    // SAFETY: bufor wyjściowy o rozmiarze `TOKEN_ELEVATION`.
    unsafe {
        GetTokenInformation(
            t.raw(),
            TokenElevation,
            Some((&raw mut e).cast::<c_void>()),
            u32::try_from(size_of::<TOKEN_ELEVATION>()).unwrap_or(0),
            &raw mut ret,
        )
    }
    .ok()?;
    Some(e.TokenIsElevated != 0)
}

/// SID właściciela procesu (bajty).
fn user_sid(h: HANDLE) -> Option<Vec<u8>> {
    let t = token(h)?;
    let mut len = 0u32;
    // SAFETY: zapytanie o rozmiar (bez bufora).
    let _ = unsafe { GetTokenInformation(t.raw(), TokenUser, None, 0, &raw mut len) };
    if len == 0 || len > 4_096 {
        return None;
    }
    // Bufor wyrównany do `u64` (struktura `TOKEN_USER` zawiera wskaźnik).
    let mut buf = vec![0u64; (len as usize).div_ceil(8)];
    // SAFETY: bufor ma co najmniej `len` bajtów.
    unsafe {
        GetTokenInformation(
            t.raw(),
            TokenUser,
            Some(buf.as_mut_ptr().cast::<c_void>()),
            len,
            &raw mut len,
        )
    }
    .ok()?;
    // SAFETY: bufor wypełniony przez system strukturą `TOKEN_USER` z wyrównaniem `u64`.
    let user = unsafe { &*buf.as_ptr().cast::<TOKEN_USER>() };
    let sid = user.User.Sid;
    // SAFETY: SID wskazuje wnętrze bufora `buf` (żyje do końca funkcji).
    let n = unsafe { GetLengthSid(sid) } as usize;
    // SAFETY: SID ma `n` bajtów wewnątrz `buf`.
    let bytes = unsafe { std::slice::from_raw_parts(sid.0.cast::<u8>(), n) };
    Some(bytes.to_vec())
}

fn current_sid() -> Option<Vec<u8>> {
    // SAFETY: pseudo-uchwyt bieżącego procesu (nie zamyka się).
    user_sid(unsafe { GetCurrentProcess() })
}

fn session_of(pid: u32) -> Option<u32> {
    let mut s = 0u32;
    // SAFETY: wyjście do `s`.
    unsafe { ProcessIdToSessionId(pid, &raw mut s) }.ok()?;
    Some(s)
}

/// Surowa migawka Toolhelp32: (PID, rodzic, nazwa obrazu, wątki).
fn snapshot() -> Result<Vec<(u32, u32, String, u32)>, SysError> {
    // SAFETY: migawka procesów, uchwyt przejmuje `OwnedHandle`.
    let raw = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) }
        .map_err(|e| sys_error("CreateToolhelp32Snapshot", &e))?;
    let snap = OwnedHandle::new(raw)
        .ok_or_else(|| SysError::Io("CreateToolhelp32Snapshot: pusty uchwyt".into()))?;
    let mut entry = PROCESSENTRY32W {
        dwSize: u32::try_from(size_of::<PROCESSENTRY32W>()).unwrap_or(0),
        ..Default::default()
    };
    let mut out = Vec::new();
    // SAFETY: `dwSize` ustawione; migawka ważna.
    let mut more = unsafe { Process32FirstW(snap.raw(), &raw mut entry) }.is_ok();
    while more {
        out.push((
            entry.th32ProcessID,
            entry.th32ParentProcessID,
            from_wide(&entry.szExeFile),
            entry.cntThreads,
        ));
        // SAFETY: jw.
        more = unsafe { Process32NextW(snap.raw(), &raw mut entry) }.is_ok();
    }
    Ok(out)
}

fn owner_is(pid: u32, me: Option<&[u8]>) -> Option<bool> {
    let me = me?;
    let h = open(pid, PROCESS_QUERY_LIMITED_INFORMATION).ok()?;
    user_sid(h.raw()).map(|sid| sid == me)
}

/// Procesy z właścicielem i sesją.
pub(crate) fn processes() -> Result<Vec<ProcessEntry>, SysError> {
    let me = current_sid();
    Ok(snapshot()?
        .into_iter()
        .map(|(pid, parent_pid, image, threads)| ProcessEntry {
            own: owner_is(pid, me.as_deref()).or(Some(false)),
            session_id: session_of(pid),
            pid,
            parent_pid,
            image,
            threads,
        })
        .collect())
}

/// Szczegóły procesu.
pub(crate) fn process(pid: u32) -> Result<ProcessDetails, SysError> {
    let entry = processes()?
        .into_iter()
        .find(|p| p.pid == pid)
        .ok_or_else(|| SysError::NotFound(format!("proces {pid}")))?;
    let h = open(pid, PROCESS_QUERY_LIMITED_INFORMATION).ok();
    let raw = h.as_ref().map(OwnedHandle::raw);
    Ok(ProcessDetails {
        path: raw.and_then(image_path),
        started_ms: raw.and_then(started_ms),
        memory_kb: raw.and_then(memory_kb),
        elevated: raw.and_then(elevated),
        entry,
    })
}

/// Zakończenie procesu o tożsamości `id` (sprawdzenia na uchwycie, którym kończymy).
pub(crate) fn terminate(guard: &TargetGuard, id: &ProcessIdentity) -> Result<(), SysError> {
    let all = snapshot()?;
    let parent_of = |p: u32| all.iter().find(|e| e.0 == p).map(|e| e.1);
    let (_, _, image, _) = all
        .iter()
        .find(|e| e.0 == id.pid)
        .ok_or_else(|| SysError::NotFound(format!("proces {}", id.pid)))?;
    if protected_process(guard, id.pid, image, parent_of) {
        return Err(SysError::Protected(format!("{image} (PID {})", id.pid)));
    }
    let h = open(
        id.pid,
        PROCESS_TERMINATE | PROCESS_QUERY_LIMITED_INFORMATION,
    )?;
    let path = image_path(h.raw()).unwrap_or_default();
    if !id.matches(&path, started_ms(h.raw())) {
        return Err(SysError::Changed(format!("PID {}", id.pid)));
    }
    if path.is_empty() || guard.is_protected(id.pid, &path) {
        return Err(SysError::Protected(format!("{path} (PID {})", id.pid)));
    }
    let me = current_sid();
    if me.is_none() || user_sid(h.raw()) != me || elevated(h.raw()) != Some(false) {
        return Err(SysError::Protected(format!(
            "{image} (PID {}) — proces innego użytkownika albo podniesiony",
            id.pid
        )));
    }
    // SAFETY: uchwyt z prawem `PROCESS_TERMINATE`, tożsamość sprawdzona na tym uchwycie.
    unsafe { TerminateProcess(h.raw(), EXIT_CODE) }
        .map_err(|e| sys_error(&format!("zakończenie procesu {}", id.pid), &e))
}
