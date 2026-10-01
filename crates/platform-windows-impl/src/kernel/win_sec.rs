//! Win32: deskryptory z SDDL, tożsamość procesu (obraz, SID tokenu, poziom integralności, sesja),
//! katalogi prywatne z chronionym DACL, MMCSS „Pro Audio” (RAII), wolne miejsce na woluminie.

#![allow(unsafe_code)]

use std::path::{Path, PathBuf};
use std::ptr::null_mut;

use platform_contract::{
    DiskSpace, IntegrityLevel, MmcssTask, PeerIdentity, PlatformError, Sid, SignatureStatus,
    ThreadBoost, private_dir_sddl,
};
use windows::Win32::Foundation::{HANDLE, HLOCAL, LocalFree};
use windows::Win32::Security::Authorization::{
    ConvertSidToStringSidW, ConvertStringSecurityDescriptorToSecurityDescriptorW, SDDL_REVISION_1,
    SE_FILE_OBJECT, SetNamedSecurityInfoW,
};
use windows::Win32::Security::{
    ACL, DACL_SECURITY_INFORMATION, GetSecurityDescriptorDacl, GetSidSubAuthority,
    GetSidSubAuthorityCount, GetTokenInformation, PROTECTED_DACL_SECURITY_INFORMATION,
    PSECURITY_DESCRIPTOR, PSID, SECURITY_ATTRIBUTES, TOKEN_INFORMATION_CLASS,
    TOKEN_MANDATORY_LABEL, TOKEN_QUERY, TOKEN_USER, TokenIntegrityLevel, TokenSessionId, TokenUser,
};
use windows::Win32::Storage::FileSystem::{CreateDirectoryW, GetDiskFreeSpaceExW};
use windows::Win32::System::Threading::{
    AvRevertMmThreadCharacteristics, AvSetMmThreadCharacteristicsW, GetCurrentProcess, OpenProcess,
    OpenProcessToken, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
    QueryFullProcessImageNameW,
};
use windows::core::{BOOL, PWSTR};

use crate::error::{from_hresult, from_io, hresult_from_win32};
use crate::win::{OwnedHandle, pcwstr, wide, win_error};

/// Deskryptor bezpieczeństwa z SDDL (pamięć systemu, zwalniana w `Drop`).
#[derive(Debug)]
pub(crate) struct SecurityDescriptor(PSECURITY_DESCRIPTOR);

// SAFETY: deskryptor to niezmienny blok pamięci procesu (LocalAlloc) — można go przenosić między
// wątkami; nikt go nie modyfikuje po utworzeniu.
unsafe impl Send for SecurityDescriptor {}

impl SecurityDescriptor {
    pub(crate) fn from_sddl(sddl: &str) -> Result<Self, PlatformError> {
        let text = wide(sddl);
        let mut sd = PSECURITY_DESCRIPTOR::default();
        // SAFETY: napis zakończony zerem; deskryptor alokuje system, zwalniamy go w `Drop`.
        unsafe {
            ConvertStringSecurityDescriptorToSecurityDescriptorW(
                pcwstr(&text),
                SDDL_REVISION_1,
                &raw mut sd,
                None,
            )
        }
        .map_err(|e| win_error("ConvertStringSecurityDescriptorToSecurityDescriptorW", &e))?;
        Ok(Self(sd))
    }

    pub(crate) fn attributes(&self) -> SECURITY_ATTRIBUTES {
        SECURITY_ATTRIBUTES {
            nLength: u32::try_from(size_of::<SECURITY_ATTRIBUTES>()).unwrap_or(0),
            lpSecurityDescriptor: self.0.0,
            bInheritHandle: false.into(),
        }
    }
}

impl Drop for SecurityDescriptor {
    fn drop(&mut self) {
        // SAFETY: pamięć z `ConvertStringSecurityDescriptorToSecurityDescriptorW`, zwalniana raz.
        unsafe { LocalFree(Some(HLOCAL(self.0.0))) };
    }
}

fn take_string(text: PWSTR) -> Result<String, PlatformError> {
    // SAFETY: napis zakończony zerem zaalokowany przez system.
    let s = unsafe { text.to_string() }.map_err(|e| PlatformError::Io(e.to_string()));
    // SAFETY: zwalniamy pamięć systemu dokładnie raz.
    unsafe { LocalFree(Some(HLOCAL(text.0.cast()))) };
    s
}

fn token_info(token: HANDLE, class: TOKEN_INFORMATION_CLASS) -> Result<Vec<u64>, PlatformError> {
    let mut len = 0u32;
    // SAFETY: zapytanie o rozmiar (bez bufora) — błąd „za mały bufor” jest oczekiwany.
    let _ = unsafe { GetTokenInformation(token, class, None, 0, &raw mut len) };
    let mut buf = vec![0u64; (len as usize).div_ceil(8).max(1)];
    // SAFETY: bufor wyrównany do 8 B, rozmiar ≥ `len`.
    unsafe {
        GetTokenInformation(
            token,
            class,
            Some(buf.as_mut_ptr().cast()),
            len,
            &raw mut len,
        )
    }
    .map_err(|e| win_error("GetTokenInformation", &e))?;
    Ok(buf)
}

fn sid_of(sid: PSID) -> Result<Sid, PlatformError> {
    let mut text = PWSTR::null();
    // SAFETY: SID z bufora tokenu (żyje w wywołującym); napis alokuje system.
    unsafe { ConvertSidToStringSidW(sid, &raw mut text) }
        .map_err(|e| win_error("ConvertSidToStringSidW", &e))?;
    Sid::parse(&take_string(text)?)
}

fn open_token(process: HANDLE) -> Result<OwnedHandle, PlatformError> {
    let mut raw = HANDLE::default();
    // SAFETY: token procesu tylko do odczytu.
    unsafe { OpenProcessToken(process, TOKEN_QUERY, &raw mut raw) }
        .map_err(|e| win_error("OpenProcessToken", &e))?;
    OwnedHandle::new(raw).ok_or_else(|| PlatformError::Io("OpenProcessToken: pusty uchwyt".into()))
}

fn token_user(token: HANDLE) -> Result<Sid, PlatformError> {
    let buf = token_info(token, TokenUser)?;
    // SAFETY: bufor klasy TokenUser zaczyna się od `TOKEN_USER`, jest wyrównany i żyje w zasięgu.
    let user = unsafe { &*buf.as_ptr().cast::<TOKEN_USER>() };
    sid_of(user.User.Sid)
}

fn token_integrity(token: HANDLE) -> Result<IntegrityLevel, PlatformError> {
    let buf = token_info(token, TokenIntegrityLevel)?;
    // SAFETY: bufor klasy TokenIntegrityLevel zaczyna się od `TOKEN_MANDATORY_LABEL`.
    let sid = unsafe { &*buf.as_ptr().cast::<TOKEN_MANDATORY_LABEL>() }
        .Label
        .Sid;
    // SAFETY: SID etykiety z bufora; ostatni pod-autorytet = RID poziomu (licznik ≥ 1).
    let rid = unsafe {
        let count = *GetSidSubAuthorityCount(sid);
        *GetSidSubAuthority(sid, u32::from(count.saturating_sub(1)))
    };
    Ok(IntegrityLevel::from_rid(rid))
}

/// Tożsamość procesu `pid` z jego tokenu.
pub(crate) fn identify(pid: u32) -> Result<PeerIdentity, PlatformError> {
    // SAFETY: otwarcie procesu z minimalnym prawem zapytania.
    let raw = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) }
        .map_err(|e| win_error("OpenProcess", &e))?;
    let process = OwnedHandle::new(raw)
        .ok_or_else(|| PlatformError::UnknownResource(format!("proces {pid}")))?;
    let mut buffer = vec![0u16; 1024];
    let mut len = 1024u32;
    // SAFETY: bufor 1024 znaków, długość przekazana i zwracana przez `len`.
    unsafe {
        QueryFullProcessImageNameW(
            process.raw(),
            PROCESS_NAME_WIN32,
            PWSTR(buffer.as_mut_ptr()),
            &raw mut len,
        )
    }
    .map_err(|e| win_error("QueryFullProcessImageNameW", &e))?;
    let image = PathBuf::from(String::from_utf16_lossy(
        &buffer[..(len as usize).min(1024)],
    ));
    let token = open_token(process.raw())?;
    let session = token_info(token.raw(), TokenSessionId)?;
    Ok(PeerIdentity {
        pid,
        image,
        user: token_user(token.raw())?,
        integrity: token_integrity(token.raw())?,
        session: u32::try_from(session[0] & 0xFFFF_FFFF).unwrap_or(u32::MAX),
        signature: SignatureStatus::NotVerified,
    })
}

/// SID konta bieżącego procesu.
pub(crate) fn current_user() -> Result<Sid, PlatformError> {
    // SAFETY: pseudo-uchwyt bieżącego procesu (nie wymaga zamykania).
    let token = open_token(unsafe { GetCurrentProcess() })?;
    token_user(token.raw())
}

/// Katalog prywatny: nowy — `CreateDirectoryW` z deskryptorem (bez okna wyścigu), istniejący —
/// `SetNamedSecurityInfoW` z chronionym DACL (propagowany do plików).
pub(crate) fn ensure_private_dir(path: &Path, owner: &Sid) -> Result<(), PlatformError> {
    let sd = SecurityDescriptor::from_sddl(&private_dir_sddl(owner))?;
    let w = wide(path);
    if !path.exists() {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| from_io(&e, parent))?;
        }
        let sa = sd.attributes();
        // SAFETY: ścieżka zakończona zerem; `sa` z żywym deskryptorem.
        return unsafe { CreateDirectoryW(pcwstr(&w), Some(&raw const sa)) }
            .map_err(|e| win_error("CreateDirectoryW", &e));
    }
    let (mut present, mut defaulted) = (BOOL::default(), BOOL::default());
    let mut dacl: *mut ACL = null_mut();
    // SAFETY: DACL wskazuje do wnętrza `sd`, które żyje do końca funkcji.
    unsafe { GetSecurityDescriptorDacl(sd.0, &raw mut present, &raw mut dacl, &raw mut defaulted) }
        .map_err(|e| win_error("GetSecurityDescriptorDacl", &e))?;
    let what = DACL_SECURITY_INFORMATION | PROTECTED_DACL_SECURITY_INFORMATION;
    // SAFETY: ścieżka zakończona zerem; DACL ważny (jw.).
    let err = unsafe {
        SetNamedSecurityInfoW(
            pcwstr(&w),
            SE_FILE_OBJECT,
            what,
            None,
            None,
            Some(dacl),
            None,
        )
    };
    if err.is_ok() {
        Ok(())
    } else {
        Err(from_hresult(
            "SetNamedSecurityInfoW",
            hresult_from_win32(err.0),
            "",
        ))
    }
}

/// MMCSS dla bieżącego wątku; `Drop` uchwytu przywraca priorytet (na tym samym wątku).
pub(crate) fn mmcss_boost(task: MmcssTask) -> Result<ThreadBoost, PlatformError> {
    let name = wide(task.name());
    let mut index = 0u32;
    // SAFETY: nazwa zadania zakończona zerem; dotyczy bieżącego wątku.
    let handle = unsafe { AvSetMmThreadCharacteristicsW(pcwstr(&name), &raw mut index) }
        .map_err(|e| win_error("AvSetMmThreadCharacteristicsW", &e))?;
    Ok(ThreadBoost::new(task, move || {
        // SAFETY: uchwyt z `AvSetMmThreadCharacteristicsW`; `ThreadBoost` nie jest `Send`, więc
        // zwrot następuje na tym samym wątku, dokładnie raz.
        let _ = unsafe { AvRevertMmThreadCharacteristics(handle) };
    }))
}

/// Wolne miejsce na woluminie zawierającym `path`.
pub(crate) fn free_disk_space(path: &Path) -> Result<DiskSpace, PlatformError> {
    let w = wide(path);
    let (mut available, mut total, mut free) = (0u64, 0u64, 0u64);
    // SAFETY: ścieżka zakończona zerem; wyniki do zmiennych lokalnych.
    unsafe {
        GetDiskFreeSpaceExW(
            pcwstr(&w),
            Some(&raw mut available),
            Some(&raw mut total),
            Some(&raw mut free),
        )
    }
    .map_err(|e| win_error("GetDiskFreeSpaceExW", &e))?;
    Ok(DiskSpace {
        available_bytes: available,
        total_bytes: total,
        free_bytes: free,
    })
}
