//! `RegistryPort` dla Windows: `RegOpenKeyExW(KEY_READ | KEY_WOW64_64KEY)` → wyliczenie
//! podkluczy i wartości (z limitami) → strażnik kontraktu. Deny-lista (`check_key`) działa **przed**
//! otwarciem klucza; dekodowanie danych (`REG_*` → [`RegData`]) jest przenośne i testowane.

use platform_apps_contract::{RegData, RegKey, RegListing, RegValue, RegistryError, RegistryPort};
#[cfg(windows)]
use platform_apps_contract::{check_key, guard_listing};

/// `REG_SZ`.
const REG_SZ: u32 = 1;
/// `REG_EXPAND_SZ`.
const REG_EXPAND_SZ: u32 = 2;
/// `REG_BINARY`.
const REG_BINARY: u32 = 3;
/// `REG_DWORD`.
const REG_DWORD: u32 = 4;
/// `REG_MULTI_SZ`.
const REG_MULTI_SZ: u32 = 7;
/// `REG_QWORD`.
const REG_QWORD: u32 = 11;

fn utf16(data: &[u8]) -> Vec<u16> {
    data.chunks_exact(2)
        .map(|c| u16::from_le_bytes([c[0], c[1]]))
        .collect()
}

fn wide_str(units: &[u16]) -> String {
    let end = units.iter().position(|&c| c == 0).unwrap_or(units.len());
    String::from_utf16_lossy(&units[..end])
}

/// Dane `REG_*` → [`RegData`] (napisy UTF-16 LE bez końcowych zer, liczby little-endian).
pub fn decode_value(kind: u32, data: &[u8]) -> RegData {
    match kind {
        REG_SZ => RegData::String(wide_str(&utf16(data))),
        REG_EXPAND_SZ => RegData::ExpandString(wide_str(&utf16(data))),
        REG_MULTI_SZ => RegData::MultiString(
            utf16(data)
                .split(|&c| c == 0)
                .filter(|s| !s.is_empty())
                .map(String::from_utf16_lossy)
                .collect(),
        ),
        REG_DWORD if data.len() >= 4 => {
            RegData::Dword(u32::from_le_bytes([data[0], data[1], data[2], data[3]]))
        }
        REG_QWORD if data.len() >= 8 => {
            let mut b = [0u8; 8];
            b.copy_from_slice(&data[..8]);
            RegData::Qword(u64::from_le_bytes(b))
        }
        REG_BINARY => RegData::binary(data),
        other => RegData::Other {
            kind: other,
            bytes: data.len(),
        },
    }
}

/// Rejestr Windows tylko do odczytu.
#[derive(Debug, Default, Clone, Copy)]
pub struct WinRegistry;

impl WinRegistry {
    /// Nowy port.
    pub fn new() -> Self {
        Self
    }
}

#[cfg(windows)]
impl RegistryPort for WinRegistry {
    fn list(&self, key: &RegKey, max_entries: usize) -> Result<RegListing, RegistryError> {
        check_key(key)?;
        let raw = win::list(key, max_entries)?;
        Ok(guard_listing(key, raw, max_entries))
    }

    fn read_value(&self, key: &RegKey, name: &str) -> Result<RegValue, RegistryError> {
        check_key(key)?;
        if name.chars().any(char::is_control) || name.chars().count() > 16_383 {
            return Err(RegistryError::InvalidKey(name.chars().take(64).collect()));
        }
        win::read_value(key, name).map(RegValue::guarded)
    }
}

#[cfg(not(windows))]
impl RegistryPort for WinRegistry {
    fn list(&self, key: &RegKey, _max_entries: usize) -> Result<RegListing, RegistryError> {
        Err(RegistryError::Unsupported(format!(
            "rejestr Windows niedostępny na tej platformie ({key})"
        )))
    }

    fn read_value(&self, key: &RegKey, _name: &str) -> Result<RegValue, RegistryError> {
        Err(RegistryError::Unsupported(format!(
            "rejestr Windows niedostępny na tej platformie ({key})"
        )))
    }
}

#[cfg(windows)]
#[allow(unsafe_code)]
mod win {
    use platform_apps_contract::{
        MAX_REG_ENTRIES, RegHive, RegKey, RegListing, RegValue, RegistryError,
    };
    use windows::Win32::Foundation::{
        ERROR_ACCESS_DENIED, ERROR_FILE_NOT_FOUND, ERROR_MORE_DATA, ERROR_NO_MORE_ITEMS,
        ERROR_SUCCESS, WIN32_ERROR,
    };
    use windows::Win32::System::Registry::{
        HKEY, HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_READ, KEY_WOW64_64KEY, REG_VALUE_TYPE,
        RegCloseKey, RegEnumKeyExW, RegEnumValueW, RegOpenKeyExW, RegQueryValueExW,
    };
    use windows::core::{PCWSTR, PWSTR};

    use super::decode_value;

    /// Najdłuższe dane wartości czytane w całości (bajty).
    const MAX_DATA: usize = 1 << 20;

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

    fn map(code: WIN32_ERROR, what: &str) -> RegistryError {
        if code == ERROR_FILE_NOT_FOUND {
            RegistryError::NotFound(what.to_owned())
        } else if code == ERROR_ACCESS_DENIED {
            RegistryError::AccessDenied(what.to_owned())
        } else {
            RegistryError::Io(format!("{what}: kod {}", code.0))
        }
    }

    fn open(key: &RegKey) -> Result<Key, RegistryError> {
        let root = match key.hive() {
            RegHive::CurrentUser => HKEY_CURRENT_USER,
            RegHive::LocalMachine => HKEY_LOCAL_MACHINE,
        };
        let sub = wide(&key.subpath());
        let mut out = HKEY::default();
        // SAFETY: bufor ścieżki zakończony zerem żyje do końca wywołania; wynik do `out`.
        let rc = unsafe {
            RegOpenKeyExW(
                root,
                PCWSTR(sub.as_ptr()),
                Some(0),
                KEY_READ | KEY_WOW64_64KEY,
                &raw mut out,
            )
        };
        if rc != ERROR_SUCCESS {
            return Err(map(rc, &key.to_string()));
        }
        Ok(Key(out))
    }

    /// Dane wartości o indeksie (nazwa, typ, dane) z powiększaniem bufora.
    fn enum_value(k: &Key, index: u32) -> Result<Option<(String, u32, Vec<u8>)>, RegistryError> {
        let mut name = vec![0u16; 16_384];
        let mut data = vec![0u8; 4_096];
        loop {
            let mut name_len = u32::try_from(name.len()).unwrap_or(u32::MAX);
            let mut data_len = u32::try_from(data.len()).unwrap_or(u32::MAX);
            let mut kind = 0u32;
            // SAFETY: bufory nazwy i danych o podanych długościach żyją do końca wywołania.
            let rc = unsafe {
                RegEnumValueW(
                    k.0,
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
                return Ok(None);
            }
            if rc == ERROR_MORE_DATA && data.len() < MAX_DATA {
                data.resize((data_len as usize).clamp(data.len() * 2, MAX_DATA), 0);
                continue;
            }
            if rc != ERROR_SUCCESS && rc != ERROR_MORE_DATA {
                return Err(map(rc, "wartość"));
            }
            data.truncate((data_len as usize).min(data.len()));
            let n = (name_len as usize).min(name.len());
            return Ok(Some((String::from_utf16_lossy(&name[..n]), kind, data)));
        }
    }

    pub(super) fn list(key: &RegKey, max_entries: usize) -> Result<RegListing, RegistryError> {
        let k = open(key)?;
        let max = max_entries.clamp(1, MAX_REG_ENTRIES);
        let mut subkeys = Vec::new();
        let mut truncated = false;
        for index in 0u32.. {
            let mut name = vec![0u16; 256];
            let mut len = 256u32;
            // SAFETY: bufor nazwy (255 znaków + zero) żyje do końca wywołania.
            let rc = unsafe {
                RegEnumKeyExW(
                    k.0,
                    index,
                    Some(PWSTR(name.as_mut_ptr())),
                    &raw mut len,
                    None,
                    None,
                    None,
                    None,
                )
            };
            if rc == ERROR_NO_MORE_ITEMS {
                break;
            }
            if rc != ERROR_SUCCESS {
                return Err(map(rc, &key.to_string()));
            }
            if subkeys.len() >= max * 4 {
                truncated = true;
                break;
            }
            subkeys.push(String::from_utf16_lossy(&name[..(len as usize).min(256)]));
        }
        let mut values = Vec::new();
        for index in 0u32.. {
            if values.len() >= max {
                truncated = true;
                break;
            }
            match enum_value(&k, index)? {
                Some((name, kind, data)) => values.push(RegValue {
                    name,
                    data: decode_value(kind, &data),
                }),
                None => break,
            }
        }
        Ok(RegListing {
            key: key.to_string(),
            subkeys,
            values,
            hidden_subkeys: 0,
            truncated,
        })
    }

    pub(super) fn read_value(key: &RegKey, name: &str) -> Result<RegValue, RegistryError> {
        let k = open(key)?;
        let wname = wide(name);
        let mut data = vec![0u8; 4_096];
        loop {
            let mut len = u32::try_from(data.len()).unwrap_or(u32::MAX);
            let mut kind = REG_VALUE_TYPE::default();
            // SAFETY: nazwa zakończona zerem i bufor danych o długości `len` żyją do końca wywołania.
            let rc = unsafe {
                RegQueryValueExW(
                    k.0,
                    PCWSTR(wname.as_ptr()),
                    None,
                    Some(&raw mut kind),
                    Some(data.as_mut_ptr()),
                    Some(&raw mut len),
                )
            };
            if rc == ERROR_MORE_DATA && data.len() < MAX_DATA {
                data.resize((len as usize).clamp(data.len() * 2, MAX_DATA), 0);
                continue;
            }
            if rc != ERROR_SUCCESS {
                return Err(map(rc, &format!("{key} → {name}")));
            }
            data.truncate((len as usize).min(data.len()));
            return Ok(RegValue {
                name: name.to_owned(),
                data: decode_value(kind.0, &data),
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn w(s: &str) -> Vec<u8> {
        s.encode_utf16()
            .chain(Some(0))
            .flat_map(u16::to_le_bytes)
            .collect()
    }

    #[test]
    fn decodes_registry_types() {
        assert_eq!(
            decode_value(1, &w("Notatnik")),
            RegData::String("Notatnik".into())
        );
        assert_eq!(
            decode_value(2, &w("%USERPROFILE%\\x")),
            RegData::ExpandString("%USERPROFILE%\\x".into())
        );
        let mut multi = w("a");
        multi.extend(w("bc"));
        multi.extend([0, 0]);
        assert_eq!(
            decode_value(7, &multi),
            RegData::MultiString(vec!["a".into(), "bc".into()])
        );
        assert_eq!(decode_value(4, &7u32.to_le_bytes()), RegData::Dword(7));
        assert_eq!(decode_value(11, &9u64.to_le_bytes()), RegData::Qword(9));
        assert!(
            matches!(decode_value(3, &[1, 2]), RegData::Binary { bytes: 2, hex } if hex == "0102")
        );
        assert_eq!(decode_value(4, &[1]), RegData::Other { kind: 4, bytes: 1 });
    }

    #[test]
    fn port_is_safe_off_windows() {
        let key = RegKey::parse(r"HKCU\Software").unwrap_or_else(|e| panic!("{e}"));
        let r = WinRegistry::new();
        if !cfg!(windows) {
            assert!(matches!(
                r.list(&key, 10),
                Err(RegistryError::Unsupported(_))
            ));
            assert!(r.read_value(&key, "x").is_err());
        }
        let secret =
            RegKey::parse(r"HKLM\SECURITY\Policy\Secrets").unwrap_or_else(|e| panic!("{e}"));
        if cfg!(windows) {
            assert!(matches!(r.list(&secret, 10), Err(RegistryError::Denied(_))));
        }
    }
}
