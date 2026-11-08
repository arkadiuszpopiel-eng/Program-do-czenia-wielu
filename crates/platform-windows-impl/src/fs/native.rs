//! Wywołania Win32 dla FS: przeniesienie bez nadpisania (`MoveFileExW`) i znane foldery
//! (`SHGetKnownFolderPath`).

#![allow(unsafe_code)]

use std::io;
use std::path::{Path, PathBuf};

use platform_contract::KnownFolder;
use windows::Win32::Storage::FileSystem::{
    MOVEFILE_COPY_ALLOWED, MOVEFILE_WRITE_THROUGH, MoveFileExW,
};
use windows::Win32::System::Com::CoTaskMemFree;
use windows::Win32::UI::Shell::{
    FOLDERID_LocalAppData, FOLDERID_Profile, FOLDERID_RoamingAppData, KF_FLAG_DEFAULT,
    SHGetKnownFolderPath,
};

use crate::win::{io_from_win, pcwstr, wide};

/// `MoveFileExW` bez `MOVEFILE_REPLACE_EXISTING`: atomowo odmawia, gdy cel istnieje;
/// między woluminami kopiuje i usuwa źródło (tylko pliki).
pub(crate) fn move_no_replace(from: &Path, to: &Path) -> io::Result<()> {
    let src = wide(from);
    let dst = wide(to);
    // SAFETY: oba bufory są zakończone zerem i żyją do końca wywołania.
    unsafe {
        MoveFileExW(
            pcwstr(&src),
            pcwstr(&dst),
            MOVEFILE_COPY_ALLOWED | MOVEFILE_WRITE_THROUGH,
        )
    }
    .map_err(|e| io_from_win(&e))
}

/// Ścieżka znanego folderu z powłoki (`None`, gdy system odmówi).
pub(crate) fn known_folder(folder: KnownFolder) -> Option<PathBuf> {
    let id = match folder {
        KnownFolder::LocalAppData => FOLDERID_LocalAppData,
        KnownFolder::RoamingAppData => FOLDERID_RoamingAppData,
        KnownFolder::Home => FOLDERID_Profile,
        KnownFolder::Temp => return Some(std::env::temp_dir()),
    };
    // SAFETY: `id` to stały GUID; brak tokenu = bieżący użytkownik.
    let raw = unsafe { SHGetKnownFolderPath(&id, KF_FLAG_DEFAULT, None) }.ok()?;
    // SAFETY: `raw` to napis zakończony zerem zaalokowany przez powłokę; kopiujemy go przed zwolnieniem.
    let text = unsafe { raw.to_string() }.ok();
    // SAFETY: bufor pochodzi z `SHGetKnownFolderPath` i musi być zwolniony `CoTaskMemFree` dokładnie raz.
    unsafe { CoTaskMemFree(Some(raw.0.cast_const().cast())) };
    text.map(PathBuf::from)
}
