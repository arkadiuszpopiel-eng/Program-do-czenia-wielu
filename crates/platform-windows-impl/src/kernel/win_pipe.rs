//! Named pipe z DACL (Win32): `CreateNamedPipeW` z deskryptorem z SDDL (`PipeSecurity::sddl`),
//! tryb bajtowy, `PIPE_REJECT_REMOTE_CLIENTS`, pierwsza instancja z `FILE_FLAG_FIRST_PIPE_INSTANCE`
//! (zajęta nazwa = możliwe przejęcie → błąd); zawsze jedna wolna instancja czeka, więc nazwa nie
//! wraca do puli. Klient: minimalne prawa (bez `FILE_CREATE_PIPE_INSTANCE`) i
//! `SECURITY_SQOS_PRESENT | SECURITY_IDENTIFICATION` (serwer nie podszyje się pod klienta).

#![allow(unsafe_code)]

use std::io::{self, Read, Write};
use std::time::{Duration, Instant};

use platform_contract::{
    CLIENT_ACCESS_MASK, PIPE_PREFIX, PipeConnection, PipeListener, PipeSecurity, PlatformError,
    validate_pipe_name,
};
use windows::Win32::Foundation::{ERROR_BROKEN_PIPE, ERROR_PIPE_BUSY, ERROR_PIPE_CONNECTED};
use windows::Win32::Storage::FileSystem::{
    CreateFileW, FILE_FLAG_FIRST_PIPE_INSTANCE, FILE_SHARE_NONE, OPEN_EXISTING, PIPE_ACCESS_DUPLEX,
    ReadFile, SECURITY_IDENTIFICATION, SECURITY_SQOS_PRESENT, WriteFile,
};
use windows::Win32::System::Pipes::{
    ConnectNamedPipe, CreateNamedPipeW, GetNamedPipeClientProcessId, GetNamedPipeServerProcessId,
    PIPE_READMODE_BYTE, PIPE_REJECT_REMOTE_CLIENTS, PIPE_TYPE_BYTE, PIPE_UNLIMITED_INSTANCES,
    PIPE_WAIT, WaitNamedPipeW,
};
use windows::core::Error as WinError;

use super::win_sec::SecurityDescriptor;
use crate::error::hresult_from_win32;
use crate::win::{OwnedHandle, hresult_bits, io_from_win, last_error, pcwstr, wide, win_error};

const BUFFER: u32 = 64 * 1024;

fn is_win32(e: &WinError, code: u32) -> bool {
    hresult_bits(e) == hresult_from_win32(code)
}

/// Połączenie (uchwyt synchroniczny; protokół żądanie → odpowiedź).
#[derive(Debug)]
pub(crate) struct WinPipeConnection {
    handle: OwnedHandle,
    peer: u32,
}

impl Read for WinPipeConnection {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let len = buf.len().min(BUFFER as usize);
        let mut n = 0u32;
        // SAFETY: bufor należy do nas przez całe wywołanie; uchwyt bez OVERLAPPED (synchroniczny).
        let read = unsafe {
            ReadFile(
                self.handle.raw(),
                Some(&mut buf[..len]),
                Some(&raw mut n),
                None,
            )
        };
        match read {
            Ok(()) => Ok(n as usize),
            Err(e) if is_win32(&e, ERROR_BROKEN_PIPE.0) => Ok(0),
            Err(e) => Err(io_from_win(&e)),
        }
    }
}

impl Write for WinPipeConnection {
    fn write(&mut self, data: &[u8]) -> io::Result<usize> {
        let len = data.len().min(BUFFER as usize);
        let mut n = 0u32;
        // SAFETY: dane żyją przez wywołanie; zapis synchroniczny.
        unsafe {
            WriteFile(
                self.handle.raw(),
                Some(&data[..len]),
                Some(&raw mut n),
                None,
            )
        }
        .map_err(|e| io_from_win(&e))?;
        Ok(n as usize)
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl PipeConnection for WinPipeConnection {
    fn peer_pid(&self) -> u32 {
        self.peer
    }
}

/// Nasłuch potoku z DACL.
#[derive(Debug)]
pub(crate) struct WinPipeListener {
    path: Vec<u16>,
    sd: SecurityDescriptor,
    next: Option<OwnedHandle>,
}

fn instance(
    path: &[u16],
    sd: &SecurityDescriptor,
    first: bool,
) -> Result<OwnedHandle, PlatformError> {
    let sa = sd.attributes();
    let mut open = PIPE_ACCESS_DUPLEX;
    if first {
        open |= FILE_FLAG_FIRST_PIPE_INSTANCE;
    }
    let mode = PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT | PIPE_REJECT_REMOTE_CLIENTS;
    // SAFETY: nazwa zakończona zerem i `sa` (wskazujące na żywy deskryptor) żyją przez wywołanie.
    let h = unsafe {
        CreateNamedPipeW(
            pcwstr(path),
            open,
            mode,
            PIPE_UNLIMITED_INSTANCES,
            BUFFER,
            BUFFER,
            0,
            Some(&raw const sa),
        )
    };
    // Pierwsza instancja zajęta = ktoś przejął nazwę (squatting) → odmowa dostępu.
    OwnedHandle::new(h).ok_or_else(|| last_error("CreateNamedPipeW (pierwsza instancja?)"))
}

/// Tworzy pierwszą instancję potoku z deskryptorem z `security`.
pub(crate) fn listen(security: &PipeSecurity) -> Result<Box<dyn PipeListener>, PlatformError> {
    let path = wide(security.path());
    let sd = SecurityDescriptor::from_sddl(&security.sddl())?;
    let first = instance(&path, &sd, security.first_instance())?;
    let next = Some(first);
    Ok(Box::new(WinPipeListener { path, sd, next }))
}

impl PipeListener for WinPipeListener {
    fn accept(&mut self) -> Result<Box<dyn PipeConnection>, PlatformError> {
        let handle = match self.next.take() {
            Some(h) => h,
            None => instance(&self.path, &self.sd, false)?,
        };
        // SAFETY: synchroniczne czekanie na klienta na naszym uchwycie serwera.
        if let Err(e) = unsafe { ConnectNamedPipe(handle.raw(), None) }
            && !is_win32(&e, ERROR_PIPE_CONNECTED.0)
        {
            return Err(win_error("ConnectNamedPipe", &e));
        }
        self.next = instance(&self.path, &self.sd, false).ok();
        let mut peer = 0u32;
        // SAFETY: zapytanie o PID klienta połączonej instancji.
        unsafe { GetNamedPipeClientProcessId(handle.raw(), &raw mut peer) }
            .map_err(|e| win_error("GetNamedPipeClientProcessId", &e))?;
        Ok(Box::new(WinPipeConnection { handle, peer }))
    }
}

/// Łączy się z lokalnym potokiem `name`, czekając na wolną instancję do `timeout_ms`.
pub(crate) fn connect(
    name: &str,
    timeout_ms: u32,
) -> Result<Box<dyn PipeConnection>, PlatformError> {
    validate_pipe_name(name)?;
    let path = wide(format!("{PIPE_PREFIX}{name}"));
    let deadline = Instant::now() + Duration::from_millis(timeout_ms.into());
    loop {
        // SAFETY: nazwa zakończona zerem; prawa klienta bez tworzenia instancji; poziom
        // personifikacji Identification.
        let opened = unsafe {
            CreateFileW(
                pcwstr(&path),
                CLIENT_ACCESS_MASK,
                FILE_SHARE_NONE,
                None,
                OPEN_EXISTING,
                SECURITY_SQOS_PRESENT | SECURITY_IDENTIFICATION,
                None,
            )
        };
        match opened {
            Ok(raw) => {
                let handle =
                    OwnedHandle::new(raw).ok_or_else(|| last_error("CreateFileW (potok)"))?;
                let mut peer = 0u32;
                // SAFETY: zapytanie o PID serwera połączonego potoku.
                unsafe { GetNamedPipeServerProcessId(handle.raw(), &raw mut peer) }
                    .map_err(|e| win_error("GetNamedPipeServerProcessId", &e))?;
                return Ok(Box::new(WinPipeConnection { handle, peer }));
            }
            Err(e) if is_win32(&e, ERROR_PIPE_BUSY.0) && Instant::now() < deadline => {
                let left = deadline
                    .saturating_duration_since(Instant::now())
                    .as_millis();
                // SAFETY: czekanie na wolną instancję potoku o tej nazwie.
                let _ = unsafe {
                    WaitNamedPipeW(pcwstr(&path), u32::try_from(left).unwrap_or(u32::MAX))
                };
            }
            Err(e) => return Err(win_error("CreateFileW (potok)", &e)),
        }
    }
}
