//! Usługi (tylko Windows): `EnumServicesStatusExW` (lista ze stanem i PID-em) i sterowanie
//! `StartServiceW` / `ControlService(SERVICE_CONTROL_STOP)` z czekaniem na stan końcowy
//! (`QueryServiceStatusEx`). Brak uprawnień → `PermissionDenied` (UAC przez Brokera — v2).

#![allow(unsafe_code)]

use std::time::{Duration, Instant};

use platform_apps_contract::{ServiceCommand, ServiceEntry, ServiceState, SysError};
use windows::Win32::Foundation::{
    ERROR_MORE_DATA, ERROR_SERVICE_ALREADY_RUNNING, ERROR_SERVICE_DOES_NOT_EXIST,
    ERROR_SERVICE_NOT_ACTIVE,
};
use windows::Win32::System::Services::{
    CloseServiceHandle, ControlService, ENUM_SERVICE_STATUS_PROCESSW, EnumServicesStatusExW,
    OpenSCManagerW, OpenServiceW, QueryServiceStatusEx, SC_ENUM_PROCESS_INFO, SC_HANDLE,
    SC_MANAGER_CONNECT, SC_MANAGER_ENUMERATE_SERVICE, SC_STATUS_PROCESS_INFO, SERVICE_CONTROL_STOP,
    SERVICE_QUERY_STATUS, SERVICE_START, SERVICE_STATE_ALL, SERVICE_STATUS, SERVICE_STATUS_PROCESS,
    SERVICE_STOP, SERVICE_WIN32, StartServiceW,
};
use windows::core::{HRESULT, PCWSTR};

use crate::procs_win::sys_error;

/// Najwięcej porcji wyliczenia (ochrona przed pętlą).
const MAX_ROUNDS: usize = 64;
/// Odstęp sprawdzania stanu usługi.
const POLL: Duration = Duration::from_millis(250);

struct Sc(SC_HANDLE);

impl Drop for Sc {
    fn drop(&mut self) {
        // SAFETY: uchwyt SCM/usługi otwarty przez nas, zamykany raz.
        let _ = unsafe { CloseServiceHandle(self.0) };
    }
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}

fn is(err: &windows::core::Error, code: u32) -> bool {
    err.code() == HRESULT::from_win32(code)
}

fn manager(access: u32) -> Result<Sc, SysError> {
    // SAFETY: lokalny menedżer usług, domyślna baza.
    unsafe { OpenSCManagerW(PCWSTR::null(), PCWSTR::null(), access) }
        .map(Sc)
        .map_err(|e| sys_error("menedżer usług", &e))
}

/// Usługi Win32 we wszystkich stanach.
pub(crate) fn services() -> Result<Vec<ServiceEntry>, SysError> {
    let scm = manager(SC_MANAGER_CONNECT | SC_MANAGER_ENUMERATE_SERVICE)?;
    // Bufor wyrównany do `u64` (struktury zawierają wskaźniki); 256 KiB na porcję.
    let mut buf = vec![0u64; 32 * 1024];
    let mut resume = 0u32;
    let mut out = Vec::new();
    for _ in 0..MAX_ROUNDS {
        let (mut needed, mut count) = (0u32, 0u32);
        // SAFETY: widok bajtowy na bufor `u64` tej samej długości w bajtach.
        let bytes =
            unsafe { std::slice::from_raw_parts_mut(buf.as_mut_ptr().cast::<u8>(), buf.len() * 8) };
        // SAFETY: bufor i liczniki wyjściowe żyją do końca wywołania.
        let r = unsafe {
            EnumServicesStatusExW(
                scm.0,
                SC_ENUM_PROCESS_INFO,
                SERVICE_WIN32,
                SERVICE_STATE_ALL,
                Some(bytes),
                &raw mut needed,
                &raw mut count,
                Some(&raw mut resume),
                PCWSTR::null(),
            )
        };
        let entries = buf.as_ptr().cast::<ENUM_SERVICE_STATUS_PROCESSW>();
        for i in 0..count as usize {
            // SAFETY: system zapisał `count` struktur na początku bufora; napisy wskazują jego
            // wnętrze i żyją do następnego wywołania.
            let e = unsafe { &*entries.add(i) };
            // SAFETY: napisy zakończone zerem wewnątrz bufora.
            let name = unsafe { e.lpServiceName.to_string() }.unwrap_or_default();
            // SAFETY: jw.
            let display = unsafe { e.lpDisplayName.to_string() }.unwrap_or_default();
            let st = e.ServiceStatusProcess;
            out.push(ServiceEntry {
                name,
                display_name: display,
                state: ServiceState::from_win32(st.dwCurrentState.0),
                pid: (st.dwProcessId != 0).then_some(st.dwProcessId),
            });
        }
        match r {
            Ok(()) => return Ok(out),
            Err(e) if is(&e, ERROR_MORE_DATA.0) => {
                if count == 0 {
                    let want = (needed as usize).div_ceil(8);
                    if want <= buf.len() || want > 1 << 20 {
                        return Err(SysError::Io("wyliczenie usług: bufor".into()));
                    }
                    buf.resize(want, 0);
                }
            }
            Err(e) => return Err(sys_error("wyliczenie usług", &e)),
        }
    }
    Ok(out)
}

fn status(svc: &Sc) -> Result<SERVICE_STATUS_PROCESS, SysError> {
    let mut st = SERVICE_STATUS_PROCESS::default();
    let mut needed = 0u32;
    // SAFETY: widok bajtowy na strukturę wyjściową o jej rozmiarze.
    let bytes = unsafe {
        std::slice::from_raw_parts_mut(
            (&raw mut st).cast::<u8>(),
            std::mem::size_of::<SERVICE_STATUS_PROCESS>(),
        )
    };
    // SAFETY: bufor wyjściowy o rozmiarze struktury.
    unsafe { QueryServiceStatusEx(svc.0, SC_STATUS_PROCESS_INFO, Some(bytes), &raw mut needed) }
        .map_err(|e| sys_error("stan usługi", &e))?;
    Ok(st)
}

fn wait_for(
    svc: &Sc,
    want: ServiceState,
    deadline: Instant,
) -> Result<SERVICE_STATUS_PROCESS, SysError> {
    loop {
        let st = status(svc)?;
        if ServiceState::from_win32(st.dwCurrentState.0) == want {
            return Ok(st);
        }
        if Instant::now() >= deadline {
            return Err(SysError::Timeout(format!(
                "usługa nie osiągnęła stanu {want:?}"
            )));
        }
        std::thread::sleep(POLL);
    }
}

fn stop(svc: &Sc, deadline: Instant) -> Result<SERVICE_STATUS_PROCESS, SysError> {
    let mut st = SERVICE_STATUS::default();
    // SAFETY: struktura wyjściowa stanu.
    match unsafe { ControlService(svc.0, SERVICE_CONTROL_STOP, &raw mut st) } {
        Ok(()) => {}
        Err(e) if is(&e, ERROR_SERVICE_NOT_ACTIVE.0) => {}
        Err(e) => return Err(sys_error("zatrzymanie usługi", &e)),
    }
    wait_for(svc, ServiceState::Stopped, deadline)
}

fn start(svc: &Sc, deadline: Instant) -> Result<SERVICE_STATUS_PROCESS, SysError> {
    // SAFETY: start bez argumentów.
    match unsafe { StartServiceW(svc.0, None) } {
        Ok(()) => {}
        Err(e) if is(&e, ERROR_SERVICE_ALREADY_RUNNING.0) => {}
        Err(e) => return Err(sys_error("start usługi", &e)),
    }
    wait_for(svc, ServiceState::Running, deadline)
}

/// Polecenie dla usługi z czekaniem na stan końcowy.
pub(crate) fn control(
    name: &str,
    command: ServiceCommand,
    timeout_ms: u64,
) -> Result<ServiceEntry, SysError> {
    let scm = manager(SC_MANAGER_CONNECT)?;
    let access = SERVICE_QUERY_STATUS
        | match command {
            ServiceCommand::Start => SERVICE_START,
            ServiceCommand::Stop => SERVICE_STOP,
            ServiceCommand::Restart => SERVICE_START | SERVICE_STOP,
        };
    let w = wide(name);
    // SAFETY: nazwa zakończona zerem żyje do końca wywołania.
    let svc = match unsafe { OpenServiceW(scm.0, PCWSTR(w.as_ptr()), access) } {
        Ok(h) => Sc(h),
        Err(e) if is(&e, ERROR_SERVICE_DOES_NOT_EXIST.0) => {
            return Err(SysError::NotFound(format!("usługa {name}")));
        }
        Err(e) => return Err(sys_error(&format!("usługa {name}"), &e)),
    };
    let deadline = Instant::now() + Duration::from_millis(timeout_ms);
    let st = match command {
        ServiceCommand::Start => start(&svc, deadline)?,
        ServiceCommand::Stop => stop(&svc, deadline)?,
        ServiceCommand::Restart => {
            stop(&svc, deadline)?;
            start(&svc, deadline)?
        }
    };
    Ok(ServiceEntry {
        name: name.to_owned(),
        display_name: name.to_owned(),
        state: ServiceState::from_win32(st.dwCurrentState.0),
        pid: (st.dwProcessId != 0).then_some(st.dwProcessId),
    })
}
