//! Ścieżki Windows: postać długa zamiast nazw skróconych 8.3 (`GetLongPathNameW`). Kwarantanna
//! pobrań porównuje nią ścieżkę podaną ze ścieżką rzeczywistą — `SPOPIE~1` to ten sam katalog,
//! nie dowiązanie.

#![allow(unsafe_code)]

use std::ffi::OsString;
use std::os::windows::ffi::{OsStrExt, OsStringExt};
use std::path::{Path, PathBuf};

use windows::Win32::Storage::FileSystem::GetLongPathNameW;
use windows::core::PCWSTR;

/// Ścieżka z nazwami długimi; bez zmiany, gdy system odmówi (np. ścieżka nie istnieje).
pub(crate) fn long_form(p: &Path) -> PathBuf {
    let wide: Vec<u16> = p.as_os_str().encode_wide().chain(Some(0)).collect();
    let mut buf = vec![0u16; 1024];
    loop {
        // SAFETY: `wide` kończy się zerem i żyje do końca wywołania; `buf` ma podaną długość.
        let len = unsafe { GetLongPathNameW(PCWSTR(wide.as_ptr()), Some(&mut buf)) } as usize;
        if len == 0 {
            return p.to_path_buf();
        }
        if len <= buf.len() {
            return PathBuf::from(OsString::from_wide(&buf[..len]));
        }
        buf.resize(len, 0);
    }
}
