//! Win32: uruchomienie Broker-UI w sesji konsoli z wysoką integralnością i host usługi Windows.
//!
//! Mechanizm (ADR 3): usługa (z `SeTcbPrivilege` — LocalSystem albo konto usługi z tym
//! przywilejem nadanym przy instalacji, bramka #10) bierze token zalogowanego użytkownika
//! (`WTSGetActiveConsoleSessionId` + `WTSQueryUserToken`), duplikuje go jako token główny
//! i podnosi etykietę integralności do `High` (S-1-16-12288). Proces ma konto i grupy
//! użytkownika (bez praw administratora), ale UIPI blokuje mu SendInput/komunikaty z procesów
//! średniej i niskiej integralności (agentki). Bilet startowy idzie przez anonimowy potok stdin.
//!
//! Dziedziczenie uchwytów (przegląd #1, P-07): potok powstaje bez dziedziczenia, dziedziczny staje
//! się tylko koniec do odczytu, a `PROC_THREAD_ATTRIBUTE_HANDLE_LIST` w `STARTUPINFOEXW` ogranicza
//! dziedziczenie do **jawnej listy** (ten jeden uchwyt) — dziecko nie dostaje żadnego innego
//! dziedzicznego uchwytu usługi Brokera, nawet gdy inny wątek akurat utworzył dziedziczny uchwyt.

#![allow(unsafe_code)]

use std::ffi::c_void;
use std::sync::{Mutex, OnceLock};

use platform_contract::{IntegrityLevel, PlatformError, ServiceBody, SessionLaunch, StopSignal};
use windows::Win32::Foundation::{
    HANDLE, HANDLE_FLAG_INHERIT, HANDLE_FLAGS, HLOCAL, LocalFree, SetHandleInformation,
    WAIT_TIMEOUT,
};
use windows::Win32::Security::Authorization::ConvertStringSidToSidW;
use windows::Win32::Security::{
    DuplicateTokenEx, GetLengthSid, PSID, SID_AND_ATTRIBUTES, SecurityIdentification,
    SetTokenInformation, TOKEN_ADJUST_DEFAULT, TOKEN_ADJUST_SESSIONID, TOKEN_ASSIGN_PRIMARY,
    TOKEN_DUPLICATE, TOKEN_MANDATORY_LABEL, TOKEN_QUERY, TokenIntegrityLevel, TokenPrimary,
};
use windows::Win32::Storage::FileSystem::WriteFile;
use windows::Win32::System::Pipes::CreatePipe;
use windows::Win32::System::RemoteDesktop::{WTSGetActiveConsoleSessionId, WTSQueryUserToken};
use windows::Win32::System::Services::{
    RegisterServiceCtrlHandlerExW, SERVICE_ACCEPT_SHUTDOWN, SERVICE_ACCEPT_STOP,
    SERVICE_CONTROL_INTERROGATE, SERVICE_CONTROL_SHUTDOWN, SERVICE_CONTROL_STOP, SERVICE_RUNNING,
    SERVICE_STATUS, SERVICE_STATUS_CURRENT_STATE, SERVICE_STATUS_HANDLE, SERVICE_STOP_PENDING,
    SERVICE_STOPPED, SERVICE_TABLE_ENTRYW, SERVICE_WIN32_OWN_PROCESS, SetServiceStatus,
    StartServiceCtrlDispatcherW,
};
use windows::Win32::System::Threading::{
    CREATE_UNICODE_ENVIRONMENT, CreateProcessAsUserW, DeleteProcThreadAttributeList,
    EXTENDED_STARTUPINFO_PRESENT, InitializeProcThreadAttributeList, LPPROC_THREAD_ATTRIBUTE_LIST,
    PROC_THREAD_ATTRIBUTE_HANDLE_LIST, PROCESS_INFORMATION, STARTF_USESTDHANDLES, STARTUPINFOEXW,
    UpdateProcThreadAttribute, WaitForSingleObject,
};
use windows::core::{PCWSTR, PWSTR};

use crate::win::{OwnedHandle, last_error, pcwstr, wide, win_error};

/// `SE_GROUP_INTEGRITY` (winnt.h).
const SE_GROUP_INTEGRITY: u32 = 0x20;

/// Uchwyt uruchomionego procesu.
#[derive(Debug)]
pub(crate) struct Proc(OwnedHandle);

/// Lista atrybutów procesu w buforze wyrównanym do `usize` (API wymaga wyrównania wskaźnika;
/// `Vec<u8>` ma wyrównanie 1). Zwalniana w `Drop` (także na ścieżkach błędu).
struct AttrList(Vec<usize>);

impl AttrList {
    fn new(count: u32) -> Result<Self, PlatformError> {
        let mut size = 0usize;
        // SAFETY: pierwsze wywołanie tylko zwraca wymagany rozmiar listy (błąd jest oczekiwany).
        let _ = unsafe { InitializeProcThreadAttributeList(None, count, None, &raw mut size) };
        let words = size.div_ceil(size_of::<usize>()).max(1);
        let mut buf = vec![0usize; words];
        let mut size = words * size_of::<usize>();
        let ptr = LPPROC_THREAD_ATTRIBUTE_LIST(buf.as_mut_ptr().cast());
        // SAFETY: bufor wyrównany, o rozmiarze ≥ wymaganego; `AttrList` (i jego `Drop`) powstaje
        // dopiero po udanej inicjalizacji.
        unsafe { InitializeProcThreadAttributeList(Some(ptr), count, None, &raw mut size) }
            .map_err(|e| win_error("InitializeProcThreadAttributeList", &e))?;
        Ok(Self(buf))
    }

    fn as_ptr(&mut self) -> LPPROC_THREAD_ATTRIBUTE_LIST {
        LPPROC_THREAD_ATTRIBUTE_LIST(self.0.as_mut_ptr().cast())
    }
}

impl Drop for AttrList {
    fn drop(&mut self) {
        // SAFETY: lista zainicjalizowana w `new`, zwalniana raz.
        unsafe { DeleteProcThreadAttributeList(self.as_ptr()) };
    }
}

/// `STARTUPINFOEXW` Broker-UI: stdin = koniec potoku, pulpit interaktywny, lista atrybutów
/// z jawną listą dziedziczonych uchwytów.
fn startup_info(
    desktop: &mut [u16],
    stdin: HANDLE,
    list: LPPROC_THREAD_ATTRIBUTE_LIST,
) -> STARTUPINFOEXW {
    let mut si = STARTUPINFOEXW::default();
    si.StartupInfo.cb = u32::try_from(size_of::<STARTUPINFOEXW>()).unwrap_or(0);
    si.StartupInfo.lpDesktop = PWSTR(desktop.as_mut_ptr());
    si.StartupInfo.dwFlags = STARTF_USESTDHANDLES;
    si.StartupInfo.hStdInput = stdin;
    si.lpAttributeList = list;
    si
}

fn high_integrity_user_token() -> Result<OwnedHandle, PlatformError> {
    // SAFETY: zapytanie bez argumentów.
    let session = unsafe { WTSGetActiveConsoleSessionId() };
    if session == u32::MAX {
        return Err(PlatformError::Unsupported(
            "brak sesji konsoli (nikt nie jest zalogowany)".into(),
        ));
    }
    let mut user = HANDLE::default();
    // SAFETY: token użytkownika sesji; wymaga SeTcbPrivilege (usługa).
    unsafe { WTSQueryUserToken(session, &raw mut user) }
        .map_err(|e| win_error("WTSQueryUserToken (wymaga SeTcbPrivilege)", &e))?;
    let user = OwnedHandle::new(user).ok_or_else(|| last_error("WTSQueryUserToken"))?;
    let access = TOKEN_QUERY
        | TOKEN_DUPLICATE
        | TOKEN_ASSIGN_PRIMARY
        | TOKEN_ADJUST_DEFAULT
        | TOKEN_ADJUST_SESSIONID;
    let mut primary = HANDLE::default();
    // SAFETY: duplikat tokenu użytkownika jako token główny.
    unsafe {
        DuplicateTokenEx(
            user.raw(),
            access,
            None,
            SecurityIdentification,
            TokenPrimary,
            &raw mut primary,
        )
    }
    .map_err(|e| win_error("DuplicateTokenEx", &e))?;
    let token = OwnedHandle::new(primary).ok_or_else(|| last_error("DuplicateTokenEx"))?;
    let label = wide(IntegrityLevel::High.label_sid());
    let mut sid = PSID::default();
    // SAFETY: literał SID zakończony zerem; SID zwalniamy `LocalFree` niżej.
    unsafe { ConvertStringSidToSidW(pcwstr(&label), &raw mut sid) }
        .map_err(|e| win_error("ConvertStringSidToSidW", &e))?;
    let tml = TOKEN_MANDATORY_LABEL {
        Label: SID_AND_ATTRIBUTES {
            Sid: sid,
            Attributes: SE_GROUP_INTEGRITY,
        },
    };
    // SAFETY: `sid` ważny (z `ConvertStringSidToSidW`).
    let size = u32::try_from(size_of::<TOKEN_MANDATORY_LABEL>()).unwrap_or(0)
        + unsafe { GetLengthSid(sid) };
    // SAFETY: struktura wskazuje na ważny SID; podniesienie etykiety wymaga SeTcbPrivilege.
    let set = unsafe {
        SetTokenInformation(
            token.raw(),
            TokenIntegrityLevel,
            (&raw const tml).cast::<c_void>(),
            size,
        )
    };
    // SAFETY: SID z `ConvertStringSidToSidW`, zwalniany dokładnie raz.
    unsafe { LocalFree(Some(HLOCAL(sid.0))) };
    set.map_err(|e| win_error("SetTokenInformation(TokenIntegrityLevel=High)", &e))?;
    Ok(token)
}

/// Uruchamia `spec` w sesji konsoli z wysoką integralnością; stdin = bilet startowy.
pub(crate) fn launch_high(spec: &SessionLaunch) -> Result<(u32, Proc), PlatformError> {
    if spec.args.iter().any(|a| a.contains(['"', '\0'])) {
        return Err(PlatformError::Unsupported(
            "argumenty z cudzysłowem albo NUL".into(),
        ));
    }
    let token = high_integrity_user_token()?;
    let (mut read, mut write) = (HANDLE::default(), HANDLE::default());
    // SAFETY: anonimowy potok bez dziedziczenia (atrybuty bezpieczeństwa domyślne).
    unsafe { CreatePipe(&raw mut read, &raw mut write, None, 64 * 1024) }
        .map_err(|e| win_error("CreatePipe", &e))?;
    let read = OwnedHandle::new(read).ok_or_else(|| last_error("CreatePipe"))?;
    let write = OwnedHandle::new(write).ok_or_else(|| last_error("CreatePipe"))?;
    // SAFETY: tylko koniec do odczytu staje się dziedziczny (wymóg listy uchwytów); koniec do
    // zapisu zostaje niedziedziczny.
    unsafe {
        SetHandleInformation(
            read.raw(),
            HANDLE_FLAG_INHERIT.0,
            HANDLE_FLAGS(HANDLE_FLAG_INHERIT.0),
        )
    }
    .map_err(|e| win_error("SetHandleInformation", &e))?;
    // Jawna lista dziedziczonych uchwytów — musi żyć do usunięcia listy atrybutów.
    let inherited = [read.raw()];
    let mut attrs = AttrList::new(1)?;
    // SAFETY: lista zainicjalizowana; wartość = tablica uchwytów żyjąca dłużej niż lista.
    unsafe {
        UpdateProcThreadAttribute(
            attrs.as_ptr(),
            0,
            PROC_THREAD_ATTRIBUTE_HANDLE_LIST as usize,
            Some(inherited.as_ptr().cast::<c_void>()),
            size_of_val(&inherited),
            None,
            None,
        )
    }
    .map_err(|e| win_error("UpdateProcThreadAttribute(HANDLE_LIST)", &e))?;
    let image = spec.image.to_string_lossy().into_owned();
    let mut cmd = format!("\"{image}\"");
    for a in &spec.args {
        cmd.push_str(&format!(" \"{a}\""));
    }
    let (image_w, mut cmd_w, mut desktop) = (wide(&image), wide(&cmd), wide(r"winsta0\default"));
    let dir_w = spec.image.parent().map(wide);
    let si = startup_info(&mut desktop, read.raw(), attrs.as_ptr());
    let mut pi = PROCESS_INFORMATION::default();
    let dir = dir_w.as_deref().map_or(PCWSTR::null(), pcwstr);
    // SAFETY: wszystkie bufory żyją przez wywołanie; `bInheritHandles = TRUE` jest ograniczone
    // listą `PROC_THREAD_ATTRIBUTE_HANDLE_LIST` do końca potoku (P-07).
    let created = unsafe {
        CreateProcessAsUserW(
            Some(token.raw()),
            pcwstr(&image_w),
            Some(PWSTR(cmd_w.as_mut_ptr())),
            None,
            None,
            true,
            CREATE_UNICODE_ENVIRONMENT | EXTENDED_STARTUPINFO_PRESENT,
            None,
            dir,
            &raw const si.StartupInfo,
            &raw mut pi,
        )
    };
    drop(attrs);
    created.map_err(|e| win_error("CreateProcessAsUserW", &e))?;
    drop(OwnedHandle::new(pi.hThread));
    let process =
        OwnedHandle::new(pi.hProcess).ok_or_else(|| last_error("CreateProcessAsUserW"))?;
    drop(read);
    let mut n = 0u32;
    // SAFETY: zapis biletu (mniejszy niż bufor potoku) do naszego końca potoku.
    unsafe { WriteFile(write.raw(), Some(&spec.stdin), Some(&raw mut n), None) }
        .map_err(|e| win_error("WriteFile (stdin)", &e))?;
    Ok((pi.dwProcessId, Proc(process)))
}

/// Czy proces działa (uchwyt — bez wyścigu PID).
pub(crate) fn proc_running(p: &Proc) -> bool {
    // SAFETY: sprawdzenie stanu naszego uchwytu procesu bez czekania.
    unsafe { WaitForSingleObject(p.0.raw(), 0) == WAIT_TIMEOUT }
}

struct Host {
    name: Vec<u16>,
    body: Option<ServiceBody>,
    stop: StopSignal,
    status: Option<SERVICE_STATUS_HANDLE>,
}

// SAFETY: uchwyt statusu usługi jest ważny w całym procesie; dostęp serializuje `Mutex`.
unsafe impl Send for Host {}

static HOST: OnceLock<Mutex<Host>> = OnceLock::new();

fn host() -> Option<std::sync::MutexGuard<'static, Host>> {
    HOST.get()
        .map(|m| m.lock().unwrap_or_else(|p| p.into_inner()))
}

fn report(state: SERVICE_STATUS_CURRENT_STATE, exit_code: u32) {
    let Some(handle) = host().and_then(|h| h.status) else {
        return;
    };
    let accepted = if state == SERVICE_RUNNING {
        SERVICE_ACCEPT_STOP | SERVICE_ACCEPT_SHUTDOWN
    } else {
        0
    };
    let status = SERVICE_STATUS {
        dwServiceType: SERVICE_WIN32_OWN_PROCESS,
        dwCurrentState: state,
        dwControlsAccepted: accepted,
        dwWin32ExitCode: exit_code,
        dwWaitHint: 5_000,
        ..Default::default()
    };
    // SAFETY: uchwyt z `RegisterServiceCtrlHandlerExW`; struktura żyje przez wywołanie.
    let _ = unsafe { SetServiceStatus(handle, &raw const status) };
}

unsafe extern "system" fn control(code: u32, _: u32, _: *mut c_void, _: *mut c_void) -> u32 {
    match code {
        SERVICE_CONTROL_STOP | SERVICE_CONTROL_SHUTDOWN => {
            report(SERVICE_STOP_PENDING, 0);
            if let Some(h) = host() {
                h.stop.stop();
            }
            0
        }
        SERVICE_CONTROL_INTERROGATE => 0,
        _ => 120, // ERROR_CALL_NOT_IMPLEMENTED
    }
}

unsafe extern "system" fn service_main(_: u32, _: *mut PWSTR) {
    let Some(name) = host().map(|h| h.name.clone()) else {
        return;
    };
    // SAFETY: nazwa zakończona zerem; procedura sterująca żyje przez cały proces.
    let Ok(handle) = (unsafe { RegisterServiceCtrlHandlerExW(pcwstr(&name), Some(control), None) })
    else {
        return;
    };
    let (body, stop) = match host() {
        Some(mut h) => {
            h.status = Some(handle);
            (h.body.take(), h.stop.clone())
        }
        None => return,
    };
    report(SERVICE_RUNNING, 0);
    let failed = body.is_none_or(|b| b(stop).is_err());
    report(SERVICE_STOPPED, u32::from(failed));
}

/// Uruchamia proces jako usługę `name` (blokuje do zatrzymania). Poza menedżerem usług → błąd.
pub(crate) fn run_service(name: &str, body: ServiceBody) -> Result<(), PlatformError> {
    let fresh = Host {
        name: wide(name),
        body: Some(body),
        stop: StopSignal::new(),
        status: None,
    };
    if HOST.set(Mutex::new(fresh)).is_err() {
        return Err(PlatformError::Unsupported(
            "usługa już uruchomiona w tym procesie".into(),
        ));
    }
    let mut name_w = wide(name);
    let table = [
        SERVICE_TABLE_ENTRYW {
            lpServiceName: PWSTR(name_w.as_mut_ptr()),
            lpServiceProc: Some(service_main),
        },
        SERVICE_TABLE_ENTRYW::default(),
    ];
    // SAFETY: tabela zakończona pustym wpisem; bufory żyją do powrotu dyspozytora.
    unsafe { StartServiceCtrlDispatcherW(table.as_ptr()) }.map_err(|e| {
        win_error(
            "StartServiceCtrlDispatcherW (uruchom przez menedżer usług albo z --console)",
            &e,
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// P-07: struktura startowa Broker-UI — rozszerzona (`STARTUPINFOEXW`) z listą atrybutów,
    /// stdin = wskazany uchwyt; lista atrybutów w buforze wyrównanym.
    #[test]
    fn startup_info_is_extended_with_aligned_attribute_list() {
        let mut attrs = AttrList::new(1).unwrap();
        let list = attrs.as_ptr();
        assert_eq!(list.0 as usize % std::mem::align_of::<usize>(), 0);
        let mut desktop = wide(r"winsta0\default");
        let si = startup_info(&mut desktop, HANDLE(std::ptr::dangling_mut()), list);
        assert_eq!(si.StartupInfo.cb as usize, size_of::<STARTUPINFOEXW>());
        assert_eq!(si.lpAttributeList.0, list.0);
        assert!(si.StartupInfo.dwFlags.contains(STARTF_USESTDHANDLES));
        assert!(si.StartupInfo.hStdOutput.0.is_null() && si.StartupInfo.hStdError.0.is_null());
        let inherited = [HANDLE(std::ptr::dangling_mut())];
        // SAFETY: test — lista zainicjalizowana, wartość żyje do końca testu.
        let r = unsafe {
            UpdateProcThreadAttribute(
                list,
                0,
                PROC_THREAD_ATTRIBUTE_HANDLE_LIST as usize,
                Some(inherited.as_ptr().cast::<c_void>()),
                size_of_val(&inherited),
                None,
                None,
            )
        };
        assert!(r.is_ok());
    }
}
