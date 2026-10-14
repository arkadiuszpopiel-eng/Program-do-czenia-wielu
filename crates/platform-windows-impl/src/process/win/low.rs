//! Polecenie powłoki o niskiej integralności (dokument 25 §1, audyt A-07): token Low (S-1-16-4096),
//! katalog roboczy oznaczony etykietą Low (z dziedziczeniem), `TEMP` w `%LOCALAPPDATA%Low`. Proces
//! czyta to, co użytkownik (git, cargo, node działają), ale zapisać może tylko w katalogu roboczym
//! — każdy zapis poza nim (także przeoczony przez analizę polecenia) system odrzuca (No-Write-Up).
//! Potoki stdout/stderr przez `PROC_THREAD_ATTRIBUTE_HANDLE_LIST`: proces dziedziczy wyłącznie
//! swoje trzy uchwyty. Start wstrzymany → Job Object → wznowienie, jak przy integralności Medium.

#![allow(unsafe_code)]

use std::collections::HashSet;
use std::ffi::c_void;
use std::fs::File;
use std::mem::size_of;
use std::os::windows::io::{FromRawHandle, RawHandle};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::SystemTime;

use platform_contract::{ExecSpec, PlatformError};
use windows::Win32::Foundation::{
    HANDLE, HANDLE_FLAG_INHERIT, HLOCAL, LocalFree, SetHandleInformation,
};
use windows::Win32::Security::Authorization::{
    ConvertStringSecurityDescriptorToSecurityDescriptorW, SDDL_REVISION_1, SE_FILE_OBJECT,
    SetNamedSecurityInfoW,
};
use windows::Win32::Security::{
    ACL, GetSecurityDescriptorSacl, LABEL_SECURITY_INFORMATION, PSECURITY_DESCRIPTOR,
    SECURITY_ATTRIBUTES,
};
use windows::Win32::Storage::FileSystem::{
    CreateFileW, FILE_ATTRIBUTE_NORMAL, FILE_GENERIC_READ, FILE_SHARE_READ, FILE_SHARE_WRITE,
    OPEN_EXISTING,
};
use windows::Win32::System::JobObjects::{AssignProcessToJobObject, TerminateJobObject};
use windows::Win32::System::Pipes::CreatePipe;
use windows::Win32::System::Threading::{
    CREATE_NO_WINDOW, CREATE_SUSPENDED, CREATE_UNICODE_ENVIRONMENT, CreateProcessAsUserW,
    EXTENDED_STARTUPINFO_PRESENT, InitializeProcThreadAttributeList, LPPROC_THREAD_ATTRIBUTE_LIST,
    PROC_THREAD_ATTRIBUTE_HANDLE_LIST, PROCESS_INFORMATION, ResumeThread, STARTF_USESTDHANDLES,
    STARTUPINFOEXW, STARTUPINFOW, TerminateProcess, UpdateProcThreadAttribute,
};
use windows::core::{PCWSTR, PWSTR, w};

use super::exec::Spawned;
use super::{Child, KILLED_EXIT_CODE, create_job, job_error, low_integrity_token};
use crate::process::JobLimits;
use crate::process::exec::args_line;
use crate::win::{OwnedHandle, last_error, wide, win_error};

/// Katalogi już oznaczone w tym procesie, z czasem utworzenia katalogu: odtworzony katalog
/// (np. `rm -rf` i `mkdir`) ma inny czas i dostaje etykietę ponownie.
type Labeled = Mutex<HashSet<(PathBuf, Option<SystemTime>)>>;

fn labeled() -> &'static Labeled {
    static SET: OnceLock<Labeled> = OnceLock::new();
    SET.get_or_init(|| Mutex::new(HashSet::new()))
}

/// Katalogi zbyt szerokie na etykietę Low (trwała, przechodzi na całe drzewo): korzeń dysku,
/// profil użytkownika i każdy jego przodek, foldery profilu (Pulpit, Dokumenty, Pobrane…),
/// katalogi systemu. Dla nich polecenie idzie z integralnością Medium (za zgodami jak dotąd).
pub(crate) fn too_broad(dir: &Path) -> bool {
    let norm = |p: &Path| {
        p.to_string_lossy()
            .trim_end_matches(['\\', '/'])
            .to_lowercase()
    };
    let d = norm(dir);
    if dir.parent().is_none() || d.len() <= 3 {
        return true;
    }
    let env = |k: &str| std::env::var_os(k).map(PathBuf::from);
    if let Some(home) = env("USERPROFILE") {
        let h = norm(&home);
        if h == d || h.starts_with(&format!("{d}\\")) {
            return true;
        }
        for name in [
            "Desktop",
            "Documents",
            "Downloads",
            "Pictures",
            "Music",
            "Videos",
            "OneDrive",
            "AppData",
            "Pulpit",
            "Dokumenty",
            "Pobrane",
        ] {
            if norm(&home.join(name)) == d {
                return true;
            }
        }
    }
    [
        "SystemRoot",
        "ProgramFiles",
        "ProgramFiles(x86)",
        "ProgramData",
    ]
    .iter()
    .filter_map(|k| env(k))
    .any(|sys| {
        let s = norm(&sys);
        d == s || d.starts_with(&format!("{s}\\"))
    })
}

/// Etykieta Low z dziedziczeniem (pliki i podkatalogi) na katalogu roboczym: proces Low może tam
/// pisać; procesy Medium (użytkownik, Alfa) — bez zmian.
pub(crate) fn label_low(dir: &Path) -> Result<(), PlatformError> {
    if too_broad(dir) {
        return Err(PlatformError::Unsupported(format!(
            "katalog {} jest zbyt szeroki na izolację (profil, korzeń dysku albo katalog systemu)",
            dir.display()
        )));
    }
    let created = std::fs::metadata(dir).and_then(|m| m.created()).ok();
    let key = (dir.to_path_buf(), created);
    if labeled().lock().map(|s| s.contains(&key)).unwrap_or(false) {
        return Ok(());
    }
    let mut sd = PSECURITY_DESCRIPTOR::default();
    // SAFETY: literał SDDL zakończony zerem; deskryptor zwalniamy `LocalFree` niżej.
    unsafe {
        ConvertStringSecurityDescriptorToSecurityDescriptorW(
            w!("S:(ML;OICI;NW;;;LW)"),
            SDDL_REVISION_1,
            &raw mut sd,
            None,
        )
    }
    .map_err(job_error(
        "ConvertStringSecurityDescriptorToSecurityDescriptorW",
    ))?;
    let mut present = false.into();
    let mut defaulted = false.into();
    let mut sacl: *mut ACL = std::ptr::null_mut();
    // SAFETY: `sd` jest ważnym deskryptorem z powyższego wywołania.
    let got = unsafe {
        GetSecurityDescriptorSacl(sd, &raw mut present, &raw mut sacl, &raw mut defaulted)
    };
    let path = wide(dir);
    let set = got
        .map_err(job_error("GetSecurityDescriptorSacl"))
        .and_then(|()| {
            // SAFETY: ścieżka zakończona zerem; SACL wskazuje do wnętrza `sd`, który żyje do `LocalFree`.
            let code = unsafe {
                SetNamedSecurityInfoW(
                    PCWSTR(path.as_ptr()),
                    SE_FILE_OBJECT,
                    LABEL_SECURITY_INFORMATION,
                    None,
                    None,
                    None,
                    Some(sacl.cast_const()),
                )
            };
            if code.is_ok() {
                Ok(())
            } else {
                Err(win_error(
                    &format!("SetNamedSecurityInfoW({})", dir.display()),
                    &windows::core::Error::from(code.to_hresult()),
                ))
            }
        });
    // SAFETY: deskryptor zaalokowany przez system, zwalniany dokładnie raz.
    unsafe { LocalFree(Some(HLOCAL(sd.0))) };
    // Dysk bez ACL (exFAT, FAT32), udział sieciowy, brak prawa zmiany etykiety: izolacja
    // niedostępna — wywołujący decyduje, czy uruchomić bez niej.
    set.map_err(|e| PlatformError::Unsupported(format!("izolacja niedostępna: {e}")))?;
    if let Ok(mut s) = labeled().lock() {
        s.insert(key);
    }
    Ok(())
}

/// Katalog tymczasowy procesów Low: `%USERPROFILE%\AppData\LocalLow\Alfa\tmp` (LocalLow ma etykietę
/// Low od systemu — zwykły `%TEMP%` jest dla nich tylko do odczytu).
fn low_temp() -> Option<PathBuf> {
    let dir = PathBuf::from(std::env::var_os("USERPROFILE")?).join(r"AppData\LocalLow\Alfa\tmp");
    std::fs::create_dir_all(&dir).ok()?;
    Some(dir)
}

/// Blok środowiska UTF-16 (`K=V\0…\0\0`) z `TEMP`/`TMP` podmienionymi na katalog Low.
fn env_block(env: &[(String, String)]) -> Vec<u16> {
    let temp = low_temp();
    let mut block: Vec<u16> = Vec::new();
    for (k, v) in env {
        if temp.is_some() && (k.eq_ignore_ascii_case("TEMP") || k.eq_ignore_ascii_case("TMP")) {
            continue;
        }
        block.extend(format!("{k}={v}").encode_utf16());
        block.push(0);
    }
    if let Some(t) = &temp {
        for k in ["TEMP", "TMP"] {
            block.extend(format!("{k}={}", t.display()).encode_utf16());
            block.push(0);
        }
    }
    block.push(0);
    block
}

/// Potok (oba końce niedziedziczone; koniec procesu staje się dziedziczony dopiero pod
/// [`super::spawn_lock`], tuż przed startem).
fn pipe() -> Result<(OwnedHandle, OwnedHandle), PlatformError> {
    let sa = SECURITY_ATTRIBUTES {
        nLength: u32::try_from(size_of::<SECURITY_ATTRIBUTES>()).unwrap_or(0),
        lpSecurityDescriptor: std::ptr::null_mut(),
        bInheritHandle: false.into(),
    };
    let (mut read, mut write) = (HANDLE::default(), HANDLE::default());
    // SAFETY: wskaźniki na lokalne zmienne; uchwyty przejmują `OwnedHandle`.
    unsafe { CreatePipe(&raw mut read, &raw mut write, Some(&raw const sa), 0) }
        .map_err(job_error("CreatePipe"))?;
    let read = OwnedHandle::new(read).ok_or_else(|| last_error("CreatePipe"))?;
    let write = OwnedHandle::new(write).ok_or_else(|| last_error("CreatePipe"))?;
    Ok((read, write))
}

/// `NUL` jako wejście — część programów nie znosi pustego uchwytu stdin.
fn null_input() -> Result<OwnedHandle, PlatformError> {
    let sa = SECURITY_ATTRIBUTES {
        nLength: u32::try_from(size_of::<SECURITY_ATTRIBUTES>()).unwrap_or(0),
        lpSecurityDescriptor: std::ptr::null_mut(),
        bInheritHandle: false.into(),
    };
    // SAFETY: literał zakończony zerem; uchwyt przejmuje `OwnedHandle`.
    let h = unsafe {
        CreateFileW(
            w!("NUL"),
            FILE_GENERIC_READ.0,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            Some(&raw const sa),
            OPEN_EXISTING,
            FILE_ATTRIBUTE_NORMAL,
            None,
        )
    }
    .map_err(job_error("CreateFileW(NUL)"))?;
    OwnedHandle::new(h).ok_or_else(|| last_error("CreateFileW(NUL)"))
}

/// Lista atrybutów z listą dziedziczonych uchwytów (bufor musi żyć do `CreateProcessAsUserW`).
struct Attributes {
    buf: Vec<u8>,
    handles: Box<[HANDLE; 3]>,
}

impl Attributes {
    fn new(handles: [HANDLE; 3]) -> Result<Self, PlatformError> {
        let mut size = 0usize;
        // SAFETY: zapytanie o rozmiar (pusta lista) — błąd ERROR_INSUFFICIENT_BUFFER jest oczekiwany.
        let _ = unsafe { InitializeProcThreadAttributeList(None, 1, None, &raw mut size) };
        let mut this = Self {
            buf: vec![0u8; size.max(1)],
            handles: Box::new(handles),
        };
        let list = this.list();
        // SAFETY: bufor ma rozmiar podany przez system.
        unsafe { InitializeProcThreadAttributeList(Some(list), 1, None, &raw mut size) }
            .map_err(job_error("InitializeProcThreadAttributeList"))?;
        // SAFETY: tablica uchwytów żyje w `Box` razem z listą (do końca `Attributes`).
        unsafe {
            UpdateProcThreadAttribute(
                list,
                0,
                PROC_THREAD_ATTRIBUTE_HANDLE_LIST as usize,
                Some(this.handles.as_ptr().cast::<c_void>()),
                size_of::<[HANDLE; 3]>(),
                None,
                None,
            )
        }
        .map_err(job_error("UpdateProcThreadAttribute(HANDLE_LIST)"))?;
        Ok(this)
    }

    fn list(&mut self) -> LPPROC_THREAD_ATTRIBUTE_LIST {
        LPPROC_THREAD_ATTRIBUTE_LIST(self.buf.as_mut_ptr().cast::<c_void>())
    }
}

/// Uchwyt końca odczytu jako `File` (dla czytników stdout/stderr).
fn into_file(handle: OwnedHandle) -> File {
    let raw = handle.raw().0 as RawHandle;
    std::mem::forget(handle);
    // SAFETY: przejmujemy wyłączną własność uchwytu (`forget` wyżej — bez podwójnego zamknięcia).
    unsafe { File::from_raw_handle(raw) }
}

/// Uruchamia polecenie o niskiej integralności (wstrzymane → Job Object → wznowienie).
pub(crate) fn spawn_low(spec: &ExecSpec, limits: &JobLimits) -> Result<Spawned, PlatformError> {
    label_low(&spec.process.cwd)?;
    let token = low_integrity_token()?;
    let job = create_job(limits)?;
    let (out_read, out_write) = pipe()?;
    let (err_read, err_write) = pipe()?;
    let input = null_input()?;
    let mut attrs = Attributes::new([input.raw(), out_write.raw(), err_write.raw()])?;
    let args = spec
        .raw_args
        .clone()
        .unwrap_or_else(|| args_line(&spec.process.args));
    let program = spec.process.cmd.display().to_string();
    let mut line = wide(if args.is_empty() {
        format!("\"{program}\"")
    } else {
        format!("\"{program}\" {args}")
    });
    let app = wide(&spec.process.cmd);
    let cwd = wide(&spec.process.cwd);
    let env = env_block(&spec.env);
    let startup = STARTUPINFOEXW {
        StartupInfo: STARTUPINFOW {
            cb: u32::try_from(size_of::<STARTUPINFOEXW>()).unwrap_or(0),
            dwFlags: STARTF_USESTDHANDLES,
            hStdInput: input.raw(),
            hStdOutput: out_write.raw(),
            hStdError: err_write.raw(),
            ..Default::default()
        },
        lpAttributeList: attrs.list(),
    };
    let mut info = PROCESS_INFORMATION::default();
    let flags = CREATE_SUSPENDED
        | CREATE_UNICODE_ENVIRONMENT
        | CREATE_NO_WINDOW
        | EXTENDED_STARTUPINFO_PRESENT;
    // Uchwyty procesu stają się dziedziczone tylko pod blokadą wspólną z drugą ścieżką startu
    // (`exec.rs`) — równoległe polecenie nie odziedziczy cudzego końca potoku.
    let guard = super::spawn_lock();
    for h in [input.raw(), out_write.raw(), err_write.raw()] {
        // SAFETY: ważne uchwyty należące do nas.
        unsafe { SetHandleInformation(h, HANDLE_FLAG_INHERIT.0, HANDLE_FLAG_INHERIT) }
            .map_err(job_error("SetHandleInformation"))?;
    }
    // SAFETY: bufory zakończone zerem żyją przez wywołanie; `line` modyfikowalny (wymóg API);
    // dziedziczone są wyłącznie trzy uchwyty z listy atrybutów.
    let created = unsafe {
        CreateProcessAsUserW(
            Some(token.raw()),
            PCWSTR(app.as_ptr()),
            Some(PWSTR(line.as_mut_ptr())),
            None,
            None,
            true,
            flags,
            Some(env.as_ptr().cast::<c_void>()),
            PCWSTR(cwd.as_ptr()),
            (&raw const startup).cast::<STARTUPINFOW>(),
            &raw mut info,
        )
    };
    drop(attrs);
    // Końce procesu zamykamy u siebie (jeszcze pod blokadą) — inaczej odczyt nie zobaczy końca
    // strumienia, a kolejny start nie odziedziczy ich przypadkiem.
    drop((input, out_write, err_write));
    drop(guard);
    created.map_err(|e| {
        win_error(
            &format!("CreateProcessAsUserW({}, Low)", spec.process.cmd.display()),
            &e,
        )
    })?;
    let process = OwnedHandle::new(info.hProcess).ok_or_else(|| last_error("CreateProcess"))?;
    let thread = OwnedHandle::new(info.hThread).ok_or_else(|| last_error("CreateProcess"))?;
    // SAFETY: oba uchwyty ważne; proces wstrzymany, więc nie zdążył utworzyć potomków.
    if let Err(e) = unsafe { AssignProcessToJobObject(job.raw(), process.raw()) } {
        // SAFETY: zabijamy własny, wstrzymany proces, którego nie udało się objąć limitami.
        let _ = unsafe { TerminateProcess(process.raw(), KILLED_EXIT_CODE) };
        return Err(win_error("AssignProcessToJobObject", &e));
    }
    // SAFETY: główny wątek wstrzymanego procesu.
    if unsafe { ResumeThread(thread.raw()) } == u32::MAX {
        let err = last_error("ResumeThread");
        // SAFETY: proces nie ruszył — sprzątamy drzewo.
        let _ = unsafe { TerminateJobObject(job.raw(), KILLED_EXIT_CODE) };
        return Err(err);
    }
    Ok(Spawned {
        child: Child {
            pid: info.dwProcessId,
            job,
            process,
            killed: false,
        },
        stdout: into_file(out_read),
        stderr: into_file(err_read),
    })
}
