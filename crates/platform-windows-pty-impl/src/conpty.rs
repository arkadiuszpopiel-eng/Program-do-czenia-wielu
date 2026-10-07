//! ConPTY (Windows 10 1809+): potoki anonimowe → `CreatePseudoConsole` → proces wstrzymany
//! z `PROC_THREAD_ATTRIBUTE_PSEUDOCONSOLE` → Job Object (`KILL_ON_JOB_CLOSE`) → wznowienie.
//! Zamknięcie: `TerminateJobObject` (całe drzewo), zamknięcie wejścia, `ClosePseudoConsole` na
//! osobnym wątku (przed Windows 11 24H2 potrafi czekać na opróżnienie wyjścia).
//!
//! Przegląd #2, P2-05: lista atrybutów w buforze wyrównanym ([`crate::attrs`]); zapis wejścia
//! nie trzyma zamka uchwytu — `close()` nigdy nie czeka na blokujący `WriteFile` (zapis działa na
//! współdzielonym uchwycie, porcjami, i kończy się po zamknięciu sesji).

#![allow(unsafe_code)]

use std::ffi::{OsStr, c_void};
use std::io::{self, Read};
use std::mem::size_of;
use std::os::windows::ffi::OsStrExt;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

use platform_contract::{PlatformError, PtySession, PtySize, PtySpec};
use windows::Win32::Foundation::{CloseHandle, ERROR_BROKEN_PIPE, HANDLE, WAIT_OBJECT_0};
use windows::Win32::Storage::FileSystem::{ReadFile, WriteFile};
use windows::Win32::System::Console::{
    COORD, ClosePseudoConsole, CreatePseudoConsole, HPCON, ResizePseudoConsole,
};
use windows::Win32::System::JobObjects::{
    AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
    JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation,
    SetInformationJobObject, TerminateJobObject,
};
use windows::Win32::System::Pipes::CreatePipe;
use windows::Win32::System::Threading::{
    CREATE_SUSPENDED, CREATE_UNICODE_ENVIRONMENT, CreateProcessW, EXTENDED_STARTUPINFO_PRESENT,
    GetExitCodeProcess, LPPROC_THREAD_ATTRIBUTE_LIST, PROC_THREAD_ATTRIBUTE_PSEUDOCONSOLE,
    PROCESS_INFORMATION, ResumeThread, STARTF_USESTDHANDLES, STARTUPINFOEXW, TerminateProcess,
    UpdateProcThreadAttribute, WaitForSingleObject,
};
use windows::core::{PCWSTR, PWSTR};

use crate::attrs::AttrList;
use crate::cmdline::{command_line, environment_block};

/// Największa porcja jednego `WriteFile` (między porcjami sprawdzane jest zamknięcie sesji).
const WRITE_CHUNK: usize = 4_096;

fn err(context: &str, e: &windows::core::Error) -> PlatformError {
    PlatformError::Io(format!("{context}: 0x{:08X} {}", e.code().0, e.message()))
}

fn wide(s: &OsStr) -> Vec<u16> {
    s.encode_wide().chain(Some(0)).collect()
}

/// Uchwyt jądra zamykany w `Drop`.
struct Owned(HANDLE);

// SAFETY: uchwyt obiektu jądra jest ważny w całym procesie, niezależnie od wątku.
unsafe impl Send for Owned {}
// SAFETY: jw.; operacje na uchwycie są bezpieczne wątkowo po stronie jądra.
unsafe impl Sync for Owned {}

impl Drop for Owned {
    fn drop(&mut self) {
        if !self.0.is_invalid() {
            // SAFETY: uchwyt należy wyłącznie do nas i jest zamykany raz.
            let _ = unsafe { CloseHandle(self.0) };
        }
    }
}

/// Pseudokonsola; `Drop` = `ClosePseudoConsole` (także na ścieżkach błędu).
struct Console(HPCON);

// SAFETY: `HPCON` to nieprzezroczysty uchwyt ważny w całym procesie; używany przez jeden wątek naraz
// (resize pod muteksem sesji, zamknięcie raz — `Option::take` + `Drop`).
unsafe impl Send for Console {}

impl Drop for Console {
    fn drop(&mut self) {
        // SAFETY: jedyne zamknięcie tej pseudokonsoli (właściciel jest jeden).
        unsafe { ClosePseudoConsole(self.0) };
    }
}

fn pipe() -> Result<(Owned, Owned), PlatformError> {
    let (mut r, mut w) = (HANDLE::default(), HANDLE::default());
    // SAFETY: potok anonimowy bez dziedziczenia; uchwyty przejmuje `Owned`.
    unsafe { CreatePipe(&raw mut r, &raw mut w, None, 0) }.map_err(|e| err("CreatePipe", &e))?;
    Ok((Owned(r), Owned(w)))
}

fn coord(size: PtySize) -> COORD {
    COORD {
        X: i16::try_from(size.cols).unwrap_or(i16::MAX),
        Y: i16::try_from(size.rows).unwrap_or(i16::MAX),
    }
}

fn job() -> Result<Owned, PlatformError> {
    // SAFETY: anonimowy Job Object; limit „zabij przy zamknięciu uchwytu” — strukturę wypełniamy lokalnie.
    unsafe {
        let job =
            Owned(CreateJobObjectW(None, PCWSTR::null()).map_err(|e| err("CreateJobObjectW", &e))?);
        let mut info = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
        info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        SetInformationJobObject(
            job.0,
            JobObjectExtendedLimitInformation,
            (&raw const info).cast::<c_void>(),
            size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
        )
        .map_err(|e| err("SetInformationJobObject", &e))?;
        Ok(job)
    }
}

/// `STARTUPINFOEXW` procesu w pseudokonsoli. `STARTF_USESTDHANDLES` z pustymi uchwytami jest
/// konieczne: bez tej flagi Windows przekazuje dziecku standardowe uchwyty rodzica, gdy nie są
/// uchwytami konsoli (przekierowane do potoku/pliku — `cargo test` w CI, `tauri dev`, usługa).
/// Proces pisze wtedy do wyjścia rodzica i czyta jego wejście zamiast pseudokonsoli. Z pustymi
/// uchwytami konsola dziecka podstawia uchwyty pseudokonsoli (tak samo Windows Terminal i node-pty).
fn startup_info(list: LPPROC_THREAD_ATTRIBUTE_LIST) -> STARTUPINFOEXW {
    let mut si = STARTUPINFOEXW::default();
    si.StartupInfo.cb = size_of::<STARTUPINFOEXW>() as u32;
    si.StartupInfo.dwFlags = STARTF_USESTDHANDLES;
    si.StartupInfo.hStdInput = HANDLE::default();
    si.StartupInfo.hStdOutput = HANDLE::default();
    si.StartupInfo.hStdError = HANDLE::default();
    si.lpAttributeList = list;
    si
}

/// Sesja ConPTY.
pub(crate) struct ConPtySession {
    pid: u32,
    process: Owned,
    job: Owned,
    /// Uchwyt wejścia; zamek tylko na czas sklonowania/zdjęcia (nie na czas zapisu).
    input: Mutex<Option<Arc<Owned>>>,
    /// Kolejność zapisów (jeden piszący naraz); `close()` go nie bierze.
    writing: Mutex<()>,
    output: Mutex<Option<Owned>>,
    console: Mutex<Option<Console>>,
    closed: AtomicBool,
}

impl std::fmt::Debug for ConPtySession {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ConPtySession")
            .field("pid", &self.pid)
            .finish_non_exhaustive()
    }
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

/// Uruchamia proces w nowej pseudokonsoli i Job Object.
pub(crate) fn spawn(spec: &PtySpec) -> Result<ConPtySession, PlatformError> {
    let (in_read, in_write) = pipe()?;
    let (out_read, out_write) = pipe()?;
    // SAFETY: pseudokonsola na naszych potokach; conhost duplikuje końcówki, które zaraz zamykamy.
    let hpc = unsafe { CreatePseudoConsole(coord(spec.size), in_read.0, out_write.0, 0) }
        .map_err(|e| err("CreatePseudoConsole", &e))?;
    let console = Console(hpc);
    drop((in_read, out_write));
    let job = job()?;
    let mut attrs = AttrList::new(1)?;
    let list = attrs.as_ptr();
    // SAFETY: lista atrybutów zainicjalizowana w wyrównanym buforze; atrybut = wartość uchwytu HPCON.
    unsafe {
        UpdateProcThreadAttribute(
            list,
            0,
            PROC_THREAD_ATTRIBUTE_PSEUDOCONSOLE as usize,
            Some(hpc.0 as *const c_void),
            size_of::<HPCON>(),
            None,
            None,
        )
    }
    .map_err(|e| err("UpdateProcThreadAttribute", &e))?;
    let si = startup_info(list);
    let program = wide(spec.program.as_os_str());
    let mut cmd: Vec<u16> = command_line(&spec.program.to_string_lossy(), &spec.args)
        .encode_utf16()
        .chain(Some(0))
        .collect();
    let env = environment_block(&spec.env);
    let cwd = wide(spec.cwd.as_os_str());
    let mut pi = PROCESS_INFORMATION::default();
    // SAFETY: wszystkie bufory żyją do końca wywołania; proces startuje wstrzymany, bez dziedziczenia uchwytów.
    let created = unsafe {
        CreateProcessW(
            PCWSTR(program.as_ptr()),
            Some(PWSTR(cmd.as_mut_ptr())),
            None,
            None,
            false,
            EXTENDED_STARTUPINFO_PRESENT | CREATE_UNICODE_ENVIRONMENT | CREATE_SUSPENDED,
            Some(env.as_ptr().cast()),
            PCWSTR(cwd.as_ptr()),
            &raw const si.StartupInfo,
            &raw mut pi,
        )
    };
    drop(attrs);
    if let Err(e) = created {
        return Err(err("CreateProcessW", &e));
    }
    let (process, thread) = (Owned(pi.hProcess), Owned(pi.hThread));
    // SAFETY: proces wstrzymany → do Job Object przed wznowieniem (żaden potomek nie ucieknie).
    if let Err(e) = unsafe { AssignProcessToJobObject(job.0, process.0) } {
        // SAFETY: proces wstrzymany, nieprzypisany — kończymy go (pseudokonsolę zamyka `Drop`).
        let _ = unsafe { TerminateProcess(process.0, 1) };
        return Err(err("AssignProcessToJobObject", &e));
    }
    // SAFETY: wznowienie głównego wątku procesu z `CREATE_SUSPENDED`.
    unsafe { ResumeThread(thread.0) };
    Ok(ConPtySession {
        pid: pi.dwProcessId,
        process,
        job,
        input: Mutex::new(Some(Arc::new(in_write))),
        writing: Mutex::new(()),
        output: Mutex::new(Some(out_read)),
        console: Mutex::new(Some(console)),
        closed: AtomicBool::new(false),
    })
}

/// Czytnik wyjścia (blokujący `ReadFile`; EOF po zamknięciu pseudokonsoli).
struct OutputReader(Owned);

impl Read for OutputReader {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let mut n = 0u32;
        // SAFETY: odczyt do bufora wywołującego z potoku anonimowego (synchronicznie).
        match unsafe { ReadFile(self.0.0, Some(buf), Some(&raw mut n), None) } {
            Ok(()) => Ok(n as usize),
            Err(e) if e.code() == ERROR_BROKEN_PIPE.to_hresult() => Ok(0),
            Err(e) => Err(io::Error::other(e.message())),
        }
    }
}

impl PtySession for ConPtySession {
    fn pid(&self) -> u32 {
        self.pid
    }

    fn take_output(&self) -> Result<Box<dyn Read + Send>, PlatformError> {
        lock(&self.output)
            .take()
            .map(|h| Box::new(OutputReader(h)) as Box<dyn Read + Send>)
            .ok_or_else(|| PlatformError::Io("wyjście terminala już pobrane".into()))
    }

    fn write_input(&self, data: &[u8]) -> Result<(), PlatformError> {
        let closed = || PlatformError::Io("sesja terminala zamknięta".into());
        let _order = lock(&self.writing);
        // Zamek uchwytu tylko na czas klonu: blokujący `WriteFile` nie wstrzymuje `close()`.
        let handle = lock(&self.input).clone().ok_or_else(closed)?;
        for chunk in data.chunks(WRITE_CHUNK) {
            let mut rest = chunk;
            while !rest.is_empty() {
                if self.closed.load(Ordering::SeqCst) {
                    return Err(closed());
                }
                let mut n = 0u32;
                // SAFETY: zapis z bufora wywołującego do potoku wejścia pseudokonsoli; uchwyt
                // żyje dzięki `Arc` do końca zapisu, nawet gdy `close()` zdjął go z sesji.
                unsafe { WriteFile(handle.0, Some(rest), Some(&raw mut n), None) }
                    .map_err(|e| err("WriteFile", &e))?;
                rest = rest.get(n as usize..).unwrap_or_default();
            }
        }
        Ok(())
    }

    fn resize(&self, size: PtySize) -> Result<(), PlatformError> {
        size.validate()?;
        let guard = lock(&self.console);
        let Some(c) = guard.as_ref() else {
            return Err(PlatformError::Io("sesja terminala zamknięta".into()));
        };
        // SAFETY: zmiana rozmiaru żywej pseudokonsoli.
        unsafe { ResizePseudoConsole(c.0, coord(size)) }.map_err(|e| err("ResizePseudoConsole", &e))
    }

    fn exit_code(&self) -> Result<Option<i32>, PlatformError> {
        // SAFETY: zapytanie bez czekania o stan procesu.
        if unsafe { WaitForSingleObject(self.process.0, 0) } != WAIT_OBJECT_0 {
            return Ok(None);
        }
        let mut code = 0u32;
        // SAFETY: odczyt kodu wyjścia zakończonego procesu.
        unsafe { GetExitCodeProcess(self.process.0, &raw mut code) }
            .map_err(|e| err("GetExitCodeProcess", &e))?;
        Ok(Some(i32::from_ne_bytes(code.to_ne_bytes())))
    }

    fn close(&self) -> Result<(), PlatformError> {
        if self.closed.swap(true, Ordering::SeqCst) {
            return Ok(());
        }
        // SAFETY: zabicie całego drzewa procesów sesji.
        let killed = unsafe { TerminateJobObject(self.job.0, 1) };
        lock(&self.input).take();
        if let Some(console) = lock(&self.console).take() {
            // `ClosePseudoConsole` może czekać na opróżnienie wyjścia — osobny wątek.
            let _ = std::thread::Builder::new()
                .name("alfa-conpty-close".into())
                .spawn(move || drop(console));
        }
        killed.map_err(|e| err("TerminateJobObject", &e))
    }
}

impl Drop for ConPtySession {
    fn drop(&mut self) {
        let _ = self.close();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Przegląd #2, P2-05: zapis „zawieszony” (tu: trzymany zamek kolejności zapisów — tak jak
    /// blokujący `WriteFile`) nie wstrzymuje `close()`; po zamknięciu zapis kończy się błędem.
    #[test]
    fn close_never_waits_for_a_blocked_writer() {
        let root = std::env::var("SystemRoot").unwrap_or_else(|_| r"C:\Windows".into());
        let spec = PtySpec {
            program: std::path::PathBuf::from(format!(r"{root}\System32\cmd.exe")),
            args: vec!["/k".into()],
            cwd: std::env::temp_dir(),
            env: platform_contract::filter_env(
                std::env::vars(),
                &platform_contract::DEFAULT_ENV_ALLOWLIST,
            ),
            size: PtySize { cols: 80, rows: 25 },
        };
        let session = std::sync::Arc::new(spawn(&spec).unwrap());
        let order = lock(&session.writing);
        let (tx, rx) = std::sync::mpsc::channel();
        let s2 = session.clone();
        std::thread::spawn(move || {
            let _ = tx.send(s2.close());
        });
        let closed = rx.recv_timeout(std::time::Duration::from_secs(5));
        assert!(closed.is_ok(), "close() czekał na piszącego");
        drop(order);
        assert!(session.write_input(b"dir\r\n").is_err());
    }

    /// Regresja (CI `windows-latest`, stdout testu = potok): bez `STARTF_USESTDHANDLES` dziecko
    /// pisało do wyjścia rodzica, a pseudokonsola nie dostawała ani bajtu.
    #[test]
    fn child_never_inherits_parent_std_handles() {
        let si = startup_info(LPPROC_THREAD_ATTRIBUTE_LIST(std::ptr::null_mut()));
        assert_eq!(si.StartupInfo.cb as usize, size_of::<STARTUPINFOEXW>());
        assert!(si.StartupInfo.dwFlags.contains(STARTF_USESTDHANDLES));
        for h in [
            si.StartupInfo.hStdInput,
            si.StartupInfo.hStdOutput,
            si.StartupInfo.hStdError,
        ] {
            assert!(h.0.is_null(), "uchwyt std musi być pusty (NULL): {h:?}");
        }
    }
}
