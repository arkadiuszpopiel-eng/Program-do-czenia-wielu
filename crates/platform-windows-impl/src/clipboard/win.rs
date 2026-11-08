//! Win32 schowka: sesja `OpenClipboard` z oknem-właścicielem typu message-only (tworzonym na wątku
//! wywołującym i niszczonym po operacji), odczyt/zapis przez `HGLOBAL`, znaczniki prywatności.

#![allow(unsafe_code)]

use std::path::PathBuf;
use std::time::Duration;

use platform_contract::{ClipboardContent, PlatformError};
use windows::Win32::Foundation::{GlobalFree, HANDLE, HGLOBAL, HWND};
use windows::Win32::System::DataExchange::{
    CloseClipboard, EmptyClipboard, GetClipboardData, IsClipboardFormatAvailable, OpenClipboard,
    RegisterClipboardFormatW, SetClipboardData,
};
use windows::Win32::System::Memory::{
    GMEM_MOVEABLE, GlobalAlloc, GlobalLock, GlobalSize, GlobalUnlock,
};
use windows::Win32::UI::Shell::{DragQueryFileW, HDROP};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DestroyWindow, HWND_MESSAGE, WINDOW_EX_STYLE, WINDOW_STYLE,
};
use windows::core::{PCWSTR, w};

use super::image::{dib_to_rgba, encode_png};
use super::{Privacy, dropfiles_bytes};
use crate::win::{last_error, win_error};

const CF_DIB: u32 = 8;
const CF_UNICODETEXT: u32 = 13;
const CF_HDROP: u32 = 15;
const CF_DIBV5: u32 = 17;
/// Ile razy próbujemy otworzyć schowek, gdy trzyma go inna aplikacja.
const OPEN_ATTEMPTS: u32 = 10;

fn format(name: PCWSTR) -> u32 {
    // SAFETY: stała nazwa formatu zakończona zerem (makro `w!`).
    unsafe { RegisterClipboardFormatW(name) }
}

fn available(format: u32) -> bool {
    // SAFETY: zapytanie bez skutków ubocznych.
    format != 0 && unsafe { IsClipboardFormatAvailable(format) }.is_ok()
}

/// Otwarty schowek; zamykany (i okno niszczone) w `Drop`.
struct Session {
    owner: HWND,
}

impl Session {
    fn open() -> Result<Self, PlatformError> {
        // SAFETY: okno message-only klasy systemowej STATIC, bez menu i danych.
        let owner = unsafe {
            CreateWindowExW(
                WINDOW_EX_STYLE(0),
                w!("STATIC"),
                w!("alfa-clipboard"),
                WINDOW_STYLE(0),
                0,
                0,
                0,
                0,
                Some(HWND_MESSAGE),
                None,
                None,
                None,
            )
        }
        .map_err(|e| win_error("CreateWindowExW(schowek)", &e))?;
        for attempt in 0..OPEN_ATTEMPTS {
            // SAFETY: `owner` to nasze okno na bieżącym wątku.
            if unsafe { OpenClipboard(Some(owner)) }.is_ok() {
                return Ok(Self { owner });
            }
            if attempt + 1 < OPEN_ATTEMPTS {
                std::thread::sleep(Duration::from_millis(10));
            }
        }
        let err = last_error("OpenClipboard (schowek zajęty przez inną aplikację)");
        // SAFETY: niszczymy własne okno utworzone wyżej.
        let _ = unsafe { DestroyWindow(owner) };
        Err(err)
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        // SAFETY: schowek otwarty przez tę sesję; okno należy do nas.
        let _ = unsafe { CloseClipboard() };
        // SAFETY: jw.
        let _ = unsafe { DestroyWindow(self.owner) };
    }
}

fn global_bytes(format: u32) -> Result<Vec<u8>, PlatformError> {
    // SAFETY: schowek jest otwarty (wywołujący trzyma `Session`); uchwyt należy do systemu.
    let handle =
        unsafe { GetClipboardData(format) }.map_err(|e| win_error("GetClipboardData", &e))?;
    let global = HGLOBAL(handle.0);
    // SAFETY: `global` to ważny blok pamięci schowka.
    let size = unsafe { GlobalSize(global) };
    // SAFETY: jw.; blokada zwalniana niżej.
    let ptr = unsafe { GlobalLock(global) };
    if ptr.is_null() {
        return Err(last_error("GlobalLock"));
    }
    // SAFETY: `ptr` wskazuje na `size` bajtów zablokowanego bloku; kopiujemy przed odblokowaniem.
    let bytes = unsafe { std::slice::from_raw_parts(ptr.cast::<u8>(), size) }.to_vec();
    // SAFETY: para do `GlobalLock`.
    let _ = unsafe { GlobalUnlock(global) };
    Ok(bytes)
}

fn set_global(format: u32, bytes: &[u8]) -> Result<(), PlatformError> {
    // SAFETY: nowy ruchomy blok; własność przechodzi na system po udanym `SetClipboardData`.
    let global = unsafe { GlobalAlloc(GMEM_MOVEABLE, bytes.len().max(1)) }
        .map_err(|e| win_error("GlobalAlloc", &e))?;
    // SAFETY: blok z `GlobalAlloc`.
    let ptr = unsafe { GlobalLock(global) };
    if ptr.is_null() {
        // SAFETY: blok nie został przekazany systemowi — zwalniamy go sami.
        let _ = unsafe { GlobalFree(Some(global)) };
        return Err(last_error("GlobalLock"));
    }
    // SAFETY: blok ma co najmniej `bytes.len()` bajtów; obszary się nie nakładają.
    unsafe { std::ptr::copy_nonoverlapping(bytes.as_ptr(), ptr.cast::<u8>(), bytes.len()) };
    // SAFETY: para do `GlobalLock`.
    let _ = unsafe { GlobalUnlock(global) };
    // SAFETY: schowek otwarty i opróżniony przez tę sesję.
    if let Err(e) = unsafe { SetClipboardData(format, Some(HANDLE(global.0))) } {
        // SAFETY: system nie przejął bloku.
        let _ = unsafe { GlobalFree(Some(global)) };
        return Err(win_error("SetClipboardData", &e));
    }
    Ok(())
}

fn read_files() -> Result<Vec<PathBuf>, PlatformError> {
    // SAFETY: schowek otwarty; uchwyt należy do systemu.
    let handle =
        unsafe { GetClipboardData(CF_HDROP) }.map_err(|e| win_error("GetClipboardData", &e))?;
    let drop = HDROP(handle.0);
    // SAFETY: `0xFFFFFFFF` = zapytanie o liczbę plików.
    let count = unsafe { DragQueryFileW(drop, u32::MAX, None) };
    let mut files = Vec::new();
    for index in 0..count {
        // SAFETY: zapytanie o długość nazwy (bez bufora).
        let len = unsafe { DragQueryFileW(drop, index, None) } as usize;
        let mut buffer = vec![0u16; len + 1];
        // SAFETY: bufor ma miejsce na nazwę i zero końcowe.
        let written = unsafe { DragQueryFileW(drop, index, Some(&mut buffer)) } as usize;
        files.push(PathBuf::from(String::from_utf16_lossy(
            &buffer[..written.min(len)],
        )));
    }
    Ok(files)
}

fn is_sensitive() -> bool {
    available(format(w!("ExcludeClipboardContentFromMonitorProcessing")))
        || available(format(w!("Clipboard Viewer Ignore")))
}

/// Odczyt bieżącej zawartości (pliki > tekst > PNG > DIB).
pub(crate) fn read() -> Result<ClipboardContent, PlatformError> {
    let _session = Session::open()?;
    if is_sensitive() {
        return Err(PlatformError::PermissionDenied(
            "schowek zawiera treść oznaczoną jako poufna (np. hasło) — pominięto".into(),
        ));
    }
    if available(CF_HDROP) {
        return read_files().map(ClipboardContent::Files);
    }
    if available(CF_UNICODETEXT) {
        let bytes = global_bytes(CF_UNICODETEXT)?;
        let units: Vec<u16> = bytes
            .chunks_exact(2)
            .map(|c| u16::from_le_bytes([c[0], c[1]]))
            .take_while(|&u| u != 0)
            .collect();
        return Ok(ClipboardContent::Text(String::from_utf16_lossy(&units)));
    }
    let png = format(w!("PNG"));
    if available(png) {
        return global_bytes(png).map(ClipboardContent::ImagePng);
    }
    for dib in [CF_DIBV5, CF_DIB] {
        if available(dib) {
            let rgba = dib_to_rgba(&global_bytes(dib)?)?;
            return Ok(ClipboardContent::ImagePng(encode_png(&rgba)));
        }
    }
    Ok(ClipboardContent::Empty)
}

/// Zapis zawartości (zastępuje wszystko) + znaczniki prywatności.
pub(crate) fn write(content: &ClipboardContent, privacy: Privacy) -> Result<(), PlatformError> {
    let _session = Session::open()?;
    // SAFETY: schowek otwarty przez tę sesję.
    unsafe { EmptyClipboard() }.map_err(|e| win_error("EmptyClipboard", &e))?;
    match content {
        ClipboardContent::Empty => {}
        ClipboardContent::Text(text) => {
            let bytes: Vec<u8> = text
                .encode_utf16()
                .chain(Some(0))
                .flat_map(u16::to_le_bytes)
                .collect();
            set_global(CF_UNICODETEXT, &bytes)?;
        }
        ClipboardContent::Files(files) => {
            set_global(CF_HDROP, &dropfiles_bytes(files))?;
            // DROPEFFECT_COPY: Eksplorator wklei kopię, nie przeniesie plików.
            set_global(format(w!("Preferred DropEffect")), &1u32.to_le_bytes())?;
        }
        ClipboardContent::ImagePng(png) => set_global(format(w!("PNG")), png)?,
    }
    let zero = 0u32.to_le_bytes();
    if privacy.exclude_history {
        set_global(format(w!("CanIncludeInClipboardHistory")), &zero)?;
        set_global(format(w!("CanUploadToCloudClipboard")), &zero)?;
    }
    if privacy.sensitive {
        set_global(
            format(w!("ExcludeClipboardContentFromMonitorProcessing")),
            &zero,
        )?;
    }
    Ok(())
}
