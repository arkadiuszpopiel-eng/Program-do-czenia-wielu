//! Zmienne środowiskowe (tylko Windows): odczyt `HKCU\Environment` i
//! `HKLM\SYSTEM\CurrentControlSet\Control\Session Manager\Environment` (wartości `REG_SZ`
//! i `REG_EXPAND_SZ`), zapis/usunięcie wyłącznie w `HKCU\Environment` (wartość z `%` jako
//! `REG_EXPAND_SZ`) i rozgłoszenie `WM_SETTINGCHANGE("Environment")` z limitem czasu.

#![allow(unsafe_code)]

use platform_apps_contract::{EnvScope, SysError};
use windows::Win32::Foundation::{
    ERROR_ACCESS_DENIED, ERROR_FILE_NOT_FOUND, ERROR_MORE_DATA, ERROR_NO_MORE_ITEMS, ERROR_SUCCESS,
    LPARAM, WIN32_ERROR, WPARAM,
};
use windows::Win32::System::Registry::{
    HKEY, HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_QUERY_VALUE, KEY_READ, KEY_SET_VALUE,
    REG_EXPAND_SZ, REG_SAM_FLAGS, REG_SZ, REG_VALUE_TYPE, RegCloseKey, RegDeleteValueW,
    RegEnumValueW, RegOpenKeyExW, RegQueryValueExW, RegSetValueExW,
};
use windows::Win32::UI::WindowsAndMessaging::{
    HWND_BROADCAST, SMTO_ABORTIFHUNG, SendMessageTimeoutW, WM_SETTINGCHANGE,
};
use windows::core::{PCWSTR, PWSTR};

/// Klucz zmiennych użytkownika (w `HKCU`).
const USER_KEY: &str = "Environment";
/// Klucz zmiennych systemowych (w `HKLM`).
const MACHINE_KEY: &str = r"SYSTEM\CurrentControlSet\Control\Session Manager\Environment";
/// Najdłuższe dane wartości (bajty).
const MAX_DATA: usize = 1 << 17;

struct Key(HKEY);

impl Drop for Key {
    fn drop(&mut self) {
        // SAFETY: klucz otwarty przez `RegOpenKeyExW`, zamykany raz.
        let _ = unsafe { RegCloseKey(self.0) };
    }
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}

fn map(code: WIN32_ERROR, what: &str) -> SysError {
    if code == ERROR_FILE_NOT_FOUND {
        SysError::NotFound(what.to_owned())
    } else if code == ERROR_ACCESS_DENIED {
        SysError::PermissionDenied(what.to_owned())
    } else {
        SysError::Io(format!("{what}: kod {}", code.0))
    }
}

fn open(root: HKEY, path: &str, access: REG_SAM_FLAGS) -> Result<Key, SysError> {
    let w = wide(path);
    let mut out = HKEY::default();
    // SAFETY: ścieżka zakończona zerem; wynik do `out`.
    let rc = unsafe { RegOpenKeyExW(root, PCWSTR(w.as_ptr()), Some(0), access, &raw mut out) };
    if rc != ERROR_SUCCESS {
        return Err(map(rc, path));
    }
    Ok(Key(out))
}

fn text(kind: u32, data: &[u8]) -> Option<String> {
    if kind != REG_SZ.0 && kind != REG_EXPAND_SZ.0 {
        return None;
    }
    let units: Vec<u16> = data
        .chunks_exact(2)
        .map(|c| u16::from_le_bytes([c[0], c[1]]))
        .collect();
    let end = units.iter().position(|&u| u == 0).unwrap_or(units.len());
    Some(String::from_utf16_lossy(&units[..end]))
}

/// Zmienne zakresu użytkownika albo systemu (surowe — sekrety ukrywa wywołujący).
pub(crate) fn read(scope: EnvScope) -> Result<Vec<(String, String)>, SysError> {
    let key = match scope {
        EnvScope::Machine => open(HKEY_LOCAL_MACHINE, MACHINE_KEY, KEY_READ)?,
        _ => open(HKEY_CURRENT_USER, USER_KEY, KEY_READ)?,
    };
    let mut out = Vec::new();
    for index in 0u32..4_096 {
        let mut name = vec![0u16; 16_384];
        let mut data = vec![0u8; 4_096];
        loop {
            let mut name_len = u32::try_from(name.len()).unwrap_or(u32::MAX);
            let mut data_len = u32::try_from(data.len()).unwrap_or(u32::MAX);
            let mut kind = 0u32;
            // SAFETY: bufory nazwy i danych o podanych długościach.
            let rc = unsafe {
                RegEnumValueW(
                    key.0,
                    index,
                    Some(PWSTR(name.as_mut_ptr())),
                    &raw mut name_len,
                    None,
                    Some(&raw mut kind),
                    Some(data.as_mut_ptr()),
                    Some(&raw mut data_len),
                )
            };
            if rc == ERROR_NO_MORE_ITEMS {
                return Ok(out);
            }
            if rc == ERROR_MORE_DATA && data.len() < MAX_DATA {
                data.resize((data_len as usize).clamp(data.len() * 2, MAX_DATA), 0);
                continue;
            }
            if rc != ERROR_SUCCESS {
                return Err(map(rc, "zmienne środowiskowe"));
            }
            let n = (name_len as usize).min(name.len());
            data.truncate((data_len as usize).min(data.len()));
            if let Some(v) = text(kind, &data) {
                out.push((String::from_utf16_lossy(&name[..n]), v));
            }
            break;
        }
    }
    Ok(out)
}

/// Surowa wartość zmiennej użytkownika.
pub(crate) fn user_value(name: &str) -> Result<Option<String>, SysError> {
    let key = open(HKEY_CURRENT_USER, USER_KEY, KEY_QUERY_VALUE)?;
    let w = wide(name);
    let mut data = vec![0u8; 4_096];
    loop {
        let mut len = u32::try_from(data.len()).unwrap_or(u32::MAX);
        let mut kind = REG_VALUE_TYPE::default();
        // SAFETY: nazwa zakończona zerem, bufor danych o długości `len`.
        let rc = unsafe {
            RegQueryValueExW(
                key.0,
                PCWSTR(w.as_ptr()),
                None,
                Some(&raw mut kind),
                Some(data.as_mut_ptr()),
                Some(&raw mut len),
            )
        };
        if rc == ERROR_FILE_NOT_FOUND {
            return Ok(None);
        }
        if rc == ERROR_MORE_DATA && data.len() < MAX_DATA {
            data.resize((len as usize).clamp(data.len() * 2, MAX_DATA), 0);
            continue;
        }
        if rc != ERROR_SUCCESS {
            return Err(map(rc, name));
        }
        data.truncate((len as usize).min(data.len()));
        return Ok(text(kind.0, &data));
    }
}

fn broadcast() {
    let w = wide(USER_KEY);
    // SAFETY: rozgłoszenie z limitem czasu; napis żyje do końca wywołania.
    let _ = unsafe {
        SendMessageTimeoutW(
            HWND_BROADCAST,
            WM_SETTINGCHANGE,
            WPARAM(0),
            LPARAM(w.as_ptr() as isize),
            SMTO_ABORTIFHUNG,
            5_000,
            None,
        )
    };
}

/// Zapis (`Some`) albo usunięcie (`None`) zmiennej użytkownika; zwraca poprzednią wartość.
pub(crate) fn set_user(name: &str, value: Option<&str>) -> Result<Option<String>, SysError> {
    let previous = user_value(name)?;
    let key = open(HKEY_CURRENT_USER, USER_KEY, KEY_SET_VALUE | KEY_QUERY_VALUE)?;
    let w = wide(name);
    let rc = match value {
        Some(v) => {
            let kind = if v.contains('%') {
                REG_EXPAND_SZ
            } else {
                REG_SZ
            };
            let bytes: Vec<u8> = wide(v).iter().flat_map(|u| u.to_le_bytes()).collect();
            // SAFETY: nazwa zakończona zerem, dane UTF-16LE z zerem końcowym.
            unsafe { RegSetValueExW(key.0, PCWSTR(w.as_ptr()), None, kind, Some(&bytes)) }
        }
        None => {
            // SAFETY: nazwa zakończona zerem.
            let rc = unsafe { RegDeleteValueW(key.0, PCWSTR(w.as_ptr())) };
            if rc == ERROR_FILE_NOT_FOUND {
                ERROR_SUCCESS
            } else {
                rc
            }
        }
    };
    if rc != ERROR_SUCCESS {
        return Err(map(rc, name));
    }
    broadcast();
    Ok(previous)
}
