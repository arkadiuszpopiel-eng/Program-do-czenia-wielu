//! Klucz bazy: 32 losowe bajty, zerowane przy `drop`, nigdy nie wypisywane w `Debug`.

use std::fmt;

use zeroize::{Zeroize, Zeroizing};

use crate::error::StoreError;

/// Długość klucza w bajtach (AES-256).
pub const KEY_LEN: usize = 32;

/// Klucz surowy bazy SQLCipher (ADR 0008: bez KDF, otwarcie < 0,2 ms).
///
/// Pamięć klucza jest zerowana przy `drop`; `Debug` nie ujawnia bajtów. Klucz trafia do SQLCipher
/// jako `PRAGMA key = "x'<64 hex>'"` — tekst polecenia też jest zerowany po użyciu.
#[derive(Clone, PartialEq, Eq)]
pub struct DbKey([u8; KEY_LEN]);

impl DbKey {
    /// Klucz z gotowych bajtów (np. odczytanych z Credential Manager).
    pub fn from_bytes(bytes: [u8; KEY_LEN]) -> Self {
        Self(bytes)
    }

    /// Nowy losowy klucz z CSPRNG systemu.
    pub fn generate() -> Result<Self, StoreError> {
        let mut bytes = [0_u8; KEY_LEN];
        getrandom::fill(&mut bytes).map_err(|e| StoreError::Random(e.to_string()))?;
        let key = Self(bytes);
        bytes.zeroize();
        Ok(key)
    }

    /// Klucz z zapisu szesnastkowego (64 znaki, wielkość liter dowolna).
    pub fn from_hex(hex: &str) -> Result<Self, StoreError> {
        let raw = hex.trim().as_bytes();
        if raw.len() != KEY_LEN * 2 {
            return Err(StoreError::InvalidKey(format!(
                "oczekiwano {} znaków hex, jest {}",
                KEY_LEN * 2,
                raw.len()
            )));
        }
        let mut bytes = [0_u8; KEY_LEN];
        for (slot, pair) in bytes.iter_mut().zip(raw.chunks_exact(2)) {
            let hi = hex_value(pair[0]);
            let lo = hex_value(pair[1]);
            match (hi, lo) {
                (Some(hi), Some(lo)) => *slot = (hi << 4) | lo,
                _ => {
                    bytes.zeroize();
                    return Err(StoreError::InvalidKey("znak spoza [0-9a-fA-F]".into()));
                }
            }
        }
        let key = Self(bytes);
        bytes.zeroize();
        Ok(key)
    }

    /// Zapis szesnastkowy (małe litery), zerowany przy `drop`.
    pub fn to_hex(&self) -> Zeroizing<String> {
        const DIGITS: &[u8; 16] = b"0123456789abcdef";
        let mut out = Zeroizing::new(String::with_capacity(KEY_LEN * 2));
        for byte in &self.0 {
            out.push(char::from(DIGITS[usize::from(byte >> 4)]));
            out.push(char::from(DIGITS[usize::from(byte & 0x0f)]));
        }
        out
    }

    /// Bajty klucza (do zapisu w sejfie kluczy).
    pub fn as_bytes(&self) -> &[u8; KEY_LEN] {
        &self.0
    }

    /// Polecenie `PRAGMA key` z kluczem surowym (bez KDF), zerowane przy `drop`.
    pub(crate) fn pragma_sql(&self) -> Zeroizing<String> {
        let hex = self.to_hex();
        let mut sql = Zeroizing::new(String::with_capacity(KEY_LEN * 2 + 20));
        sql.push_str("PRAGMA key = \"x'");
        sql.push_str(&hex);
        sql.push_str("'\";");
        sql
    }
}

impl Drop for DbKey {
    fn drop(&mut self) {
        self.0.zeroize();
    }
}

impl fmt::Debug for DbKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("DbKey(<ukryty>)")
    }
}

fn hex_value(c: u8) -> Option<u8> {
    match c {
        b'0'..=b'9' => Some(c - b'0'),
        b'a'..=b'f' => Some(c - b'a' + 10),
        b'A'..=b'F' => Some(c - b'A' + 10),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_round_trip_and_debug_hides_bytes() {
        let key = DbKey::generate().unwrap();
        let hex = key.to_hex();
        assert_eq!(hex.len(), 64);
        assert_eq!(DbKey::from_hex(&hex).unwrap(), key);
        assert_eq!(DbKey::from_hex(&hex.to_uppercase()).unwrap(), key);
        assert_eq!(format!("{key:?}"), "DbKey(<ukryty>)");
        assert!(!format!("{key:?}").contains(hex.as_str()));
    }

    #[test]
    fn invalid_hex_rejected() {
        assert!(DbKey::from_hex("abc").is_err());
        assert!(DbKey::from_hex(&"zz".repeat(32)).is_err());
    }

    #[test]
    fn generated_keys_differ_and_pragma_is_raw() {
        let a = DbKey::generate().unwrap();
        let b = DbKey::generate().unwrap();
        assert_ne!(a, b);
        let sql = a.pragma_sql();
        assert!(sql.starts_with("PRAGMA key = \"x'"));
        assert!(sql.ends_with("'\";"));
        assert_eq!(DbKey::from_bytes(*a.as_bytes()), a);
    }
}
