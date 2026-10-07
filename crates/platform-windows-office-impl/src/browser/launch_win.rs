//! Uruchomienie Edge/Chrome z CDP przez potok (Windows): dwa potoki anonimowe przekazane jako
//! deskryptory CRT 3 (polecenia) i 4 (odpowiedzi) przez `STARTUPINFO.lpReserved2` (ten sam
//! mechanizm co libuv/Puppeteer), dziedziczenie ograniczone listą `PROC_THREAD_ATTRIBUTE_HANDLE_LIST`
//! do tych dwóch uchwytów, proces wstrzymany → Job Object (`KILL_ON_JOB_CLOSE`) → wznowienie.
//! Profil przygotowany przed startem: katalogi Alfy + `Default/Preferences` bez haseł i
//! autouzupełniania. Nigdy port TCP.

#![allow(unsafe_code)]

use std::ffi::c_void;
use std::fs::File;
use std::mem::size_of;
use std::os::windows::io::{FromRawHandle, OwnedHandle};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use platform_apps_contract::{BrowserError, BrowserKind, BrowserSpec, profile_preferences};
use windows::Win32::Foundation::{
    CloseHandle, HANDLE, HANDLE_FLAG_INHERIT, HANDLE_FLAGS, INVALID_HANDLE_VALUE,
    SetHandleInformation,
};
use windows::Win32::Security::SECURITY_ATTRIBUTES;
use windows::Win32::System::JobObjects::{
    AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
    JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation,
    SetInformationJobObject, TerminateJobObject,
};
use windows::Win32::System::Pipes::CreatePipe;
use windows::Win32::System::Threading::{
    CREATE_NO_WINDOW, CREATE_SUSPENDED, CreateProcessW, DeleteProcThreadAttributeList,
    EXTENDED_STARTUPINFO_PRESENT, InitializeProcThreadAttributeList, LPPROC_THREAD_ATTRIBUTE_LIST,
    PROC_THREAD_ATTRIBUTE_HANDLE_LIST, PROCESS_INFORMATION, ResumeThread, STARTUPINFOEXW,
    TerminateProcess, UpdateProcThreadAttribute,
};
use windows::core::{BOOL, PCWSTR, PWSTR};

use super::{Launched, Launcher, ProcessGuard};

/// `FOPEN | FPIPE` w tablicy deskryptorów CRT.
const CRT_PIPE: u8 = 0x01 | 0x08;
/// Serializacja naszych startów (okno, w którym uchwyty są dziedziczne).
static LAUNCH: Mutex<()> = Mutex::new(());

fn err(context: &str, e: &windows::core::Error) -> BrowserError {
    BrowserError::Protocol(format!("{context}: 0x{:08X} {}", e.code().0, e.message()))
}

struct Owned(HANDLE);

impl Drop for Owned {
    fn drop(&mut self) {
        if !self.0.is_invalid() {
            // SAFETY: uchwyt należy wyłącznie do nas i jest zamykany raz.
            let _ = unsafe { CloseHandle(self.0) };
        }
    }
}

impl Owned {
    /// Przekazuje własność do `File` (std zamknie uchwyt).
    fn into_file(self) -> File {
        let h = self.0;
        std::mem::forget(self);
        // SAFETY: przejmujemy jedyny uchwyt końcówki potoku; `forget` wyżej zapobiega podwójnemu zamknięciu.
        File::from(unsafe { OwnedHandle::from_raw_handle(h.0) })
    }
}

/// Job Object przeglądarki; zamknięcie uchwytu zabija drzewo procesów.
struct Job(Owned);

// SAFETY: uchwyt obiektu jądra jest ważny w całym procesie, niezależnie od wątku.
unsafe impl Send for Job {}

impl ProcessGuard for Job {
    fn kill(&mut self) {
        // SAFETY: nasz Job Object; zakończenie wszystkich procesów w nim.
        let _ = unsafe { TerminateJobObject(self.0.0, 1) };
    }
}

impl Drop for Job {
    fn drop(&mut self) {
        self.kill();
    }
}

/// Uruchamianie Edge/Chrome w Windows.
#[derive(Debug, Default, Clone, Copy)]
pub struct WinLauncher;

fn executable(spec: &BrowserSpec) -> Result<PathBuf, BrowserError> {
    if let Some(p) = &spec.executable {
        return Ok(p.clone());
    }
    let rel = match spec.kind {
        BrowserKind::Edge => r"Microsoft\Edge\Application\msedge.exe",
        BrowserKind::Chrome => r"Google\Chrome\Application\chrome.exe",
    };
    ["ProgramFiles(x86)", "ProgramFiles", "LOCALAPPDATA"]
        .iter()
        .filter_map(std::env::var_os)
        .map(|base| PathBuf::from(base).join(rel))
        .find(|p| p.is_file())
        .ok_or_else(|| BrowserError::NotInstalled(spec.kind.exe().into()))
}

fn prepare_profile(spec: &BrowserSpec) -> Result<(), BrowserError> {
    let io = |e: std::io::Error| BrowserError::Policy(format!("profil przeglądarki: {e}"));
    std::fs::create_dir_all(&spec.quarantine_dir).map_err(io)?;
    let default = spec.profile_dir.join("Default");
    std::fs::create_dir_all(&default).map_err(io)?;
    let prefs = default.join("Preferences");
    if !prefs.exists() {
        std::fs::write(
            &prefs,
            profile_preferences(&spec.quarantine_dir).to_string(),
        )
        .map_err(io)?;
    }
    Ok(())
}

/// Cytowanie argumentu wg reguł MSVCRT (`CommandLineToArgvW`).
fn quote(arg: &str) -> String {
    if !arg.is_empty() && !arg.contains([' ', '\t', '"']) {
        return arg.to_owned();
    }
    let mut out = String::from("\"");
    let mut slashes = 0;
    for c in arg.chars() {
        if c == '\\' {
            slashes += 1;
            continue;
        }
        let n = if c == '"' { slashes * 2 + 1 } else { slashes };
        out.extend(std::iter::repeat_n('\\', n));
        slashes = 0;
        out.push(c);
    }
    out.extend(std::iter::repeat_n('\\', slashes * 2));
    out.push('"');
    out
}

fn wide(s: &std::ffi::OsStr) -> Vec<u16> {
    use std::os::windows::ffi::OsStrExt;
    s.encode_wide().chain(Some(0)).collect()
}

/// Potok z jedną końcówką dziedziczną (`child_reads` = dziecko czyta).
fn pipe(child_reads: bool) -> Result<(Owned, Owned), BrowserError> {
    let sa = SECURITY_ATTRIBUTES {
        nLength: u32::try_from(size_of::<SECURITY_ATTRIBUTES>()).unwrap_or(0),
        lpSecurityDescriptor: std::ptr::null_mut(),
        bInheritHandle: BOOL(1),
    };
    let (mut r, mut w) = (HANDLE::default(), HANDLE::default());
    // SAFETY: potok anonimowy; uchwyty przejmuje `Owned`.
    unsafe { CreatePipe(&raw mut r, &raw mut w, Some(&raw const sa), 0) }
        .map_err(|e| err("CreatePipe", &e))?;
    let (r, w) = (Owned(r), Owned(w));
    let ours = if child_reads { &w } else { &r };
    // SAFETY: nasza końcówka przestaje być dziedziczna (dziecko dostaje tylko swoją).
    unsafe { SetHandleInformation(ours.0, HANDLE_FLAG_INHERIT.0, HANDLE_FLAGS(0)) }
        .map_err(|e| err("SetHandleInformation", &e))?;
    Ok((r, w))
}

fn job() -> Result<Job, BrowserError> {
    // SAFETY: anonimowy Job Object; strukturę limitów wypełniamy lokalnie.
    unsafe {
        let job =
            Owned(CreateJobObjectW(None, PCWSTR::null()).map_err(|e| err("CreateJobObjectW", &e))?);
        let mut info = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
        info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        SetInformationJobObject(
            job.0,
            JobObjectExtendedLimitInformation,
            (&raw const info).cast::<c_void>(),
            u32::try_from(size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>()).unwrap_or(0),
        )
        .map_err(|e| err("SetInformationJobObject", &e))?;
        Ok(Job(job))
    }
}

/// Tablica deskryptorów CRT (`lpReserved2`): liczba, flagi, uchwyty (bez wyrównania).
fn crt_table(fd3: HANDLE, fd4: HANDLE) -> Vec<u8> {
    let count = 5u32;
    let mut buf = count.to_le_bytes().to_vec();
    buf.extend([0, 0, 0, CRT_PIPE, CRT_PIPE]);
    for h in [INVALID_HANDLE_VALUE; 3].into_iter().chain([fd3, fd4]) {
        buf.extend((h.0 as usize).to_le_bytes());
    }
    buf
}

fn spawn(
    exe: &Path,
    args: &[String],
    child_in: &Owned,
    child_out: &Owned,
) -> Result<(Owned, Owned), BrowserError> {
    let mut list_size = 0usize;
    // SAFETY: pierwsze wywołanie zwraca wymagany rozmiar listy (błąd oczekiwany).
    let _ = unsafe { InitializeProcThreadAttributeList(None, 1, None, &raw mut list_size) };
    let mut attrs = vec![0u8; list_size.max(1)];
    let list = LPPROC_THREAD_ATTRIBUTE_LIST(attrs.as_mut_ptr().cast());
    let handles = [child_in.0, child_out.0];
    // SAFETY: lista w buforze o wymaganym rozmiarze; tablica uchwytów żyje do `CreateProcessW`.
    unsafe {
        InitializeProcThreadAttributeList(Some(list), 1, None, &raw mut list_size)
            .map_err(|e| err("InitializeProcThreadAttributeList", &e))?;
        UpdateProcThreadAttribute(
            list,
            0,
            PROC_THREAD_ATTRIBUTE_HANDLE_LIST as usize,
            Some(handles.as_ptr().cast()),
            size_of::<[HANDLE; 2]>(),
            None,
            None,
        )
        .map_err(|e| err("UpdateProcThreadAttribute", &e))?;
    }
    let mut table = crt_table(child_in.0, child_out.0);
    let mut si = STARTUPINFOEXW::default();
    si.StartupInfo.cb = u32::try_from(size_of::<STARTUPINFOEXW>()).unwrap_or(0);
    si.StartupInfo.cbReserved2 = u16::try_from(table.len()).unwrap_or(0);
    si.StartupInfo.lpReserved2 = table.as_mut_ptr();
    si.lpAttributeList = list;
    let program = wide(exe.as_os_str());
    let line = std::iter::once(quote(&exe.to_string_lossy()))
        .chain(args.iter().map(|a| quote(a)))
        .collect::<Vec<_>>()
        .join(" ");
    let mut cmd: Vec<u16> = line.encode_utf16().chain(Some(0)).collect();
    let mut pi = PROCESS_INFORMATION::default();
    // SAFETY: bufory (program, linia, tablica CRT, lista atrybutów) żyją do końca wywołania;
    // dziedziczone są wyłącznie uchwyty z listy.
    let created = unsafe {
        CreateProcessW(
            PCWSTR(program.as_ptr()),
            Some(PWSTR(cmd.as_mut_ptr())),
            None,
            None,
            true,
            EXTENDED_STARTUPINFO_PRESENT | CREATE_SUSPENDED | CREATE_NO_WINDOW,
            None,
            PCWSTR::null(),
            &raw const si.StartupInfo,
            &raw mut pi,
        )
    };
    // SAFETY: lista zainicjalizowana wyżej; bufor `attrs` żyje do końca funkcji.
    unsafe { DeleteProcThreadAttributeList(list) };
    created.map_err(|e| err("CreateProcessW", &e))?;
    Ok((Owned(pi.hProcess), Owned(pi.hThread)))
}

impl Launcher for WinLauncher {
    fn launch(&self, spec: &BrowserSpec, args: &[String]) -> Result<Launched, BrowserError> {
        if args
            .iter()
            .any(|a| a.starts_with("--remote-debugging-port"))
        {
            return Err(BrowserError::Policy("CDP tylko przez potok".into()));
        }
        let exe = executable(spec)?;
        prepare_profile(spec)?;
        let job = job()?;
        let (process, thread, cmd_write, ev_read) = {
            let _serial = LAUNCH.lock().unwrap_or_else(|p| p.into_inner());
            let (cmd_read, cmd_write) = pipe(true)?;
            let (ev_read, ev_write) = pipe(false)?;
            let (process, thread) = spawn(&exe, args, &cmd_read, &ev_write)?;
            drop((cmd_read, ev_write));
            (process, thread, cmd_write, ev_read)
        };
        // SAFETY: proces wstrzymany → do Job Object przed wznowieniem (żaden potomek nie ucieknie).
        if let Err(e) = unsafe { AssignProcessToJobObject(job.0.0, process.0) } {
            // SAFETY: proces wstrzymany i nieprzypisany — kończymy go.
            let _ = unsafe { TerminateProcess(process.0, 1) };
            return Err(err("AssignProcessToJobObject", &e));
        }
        // SAFETY: wznowienie głównego wątku procesu z `CREATE_SUSPENDED`.
        unsafe { ResumeThread(thread.0) };
        Ok(Launched {
            reader: Box::new(ev_read.into_file()),
            writer: Box::new(cmd_write.into_file()),
            process: Box::new(job),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quoting_and_crt_table() {
        assert_eq!(quote("--no-pings"), "--no-pings");
        assert_eq!(
            quote(r"--user-data-dir=C:\A B\p"),
            r#""--user-data-dir=C:\A B\p""#
        );
        assert_eq!(quote(r#"a"b"#), r#""a\"b""#);
        assert_eq!(quote(r"C:\x y\"), r#""C:\x y\\""#);
        let t = crt_table(
            HANDLE(std::ptr::without_provenance_mut(8)),
            HANDLE(std::ptr::without_provenance_mut(12)),
        );
        assert_eq!(t.len(), 4 + 5 + 5 * size_of::<usize>());
        assert_eq!(&t[..9], &[5, 0, 0, 0, 0, 0, 0, CRT_PIPE, CRT_PIPE]);
    }
}
