//! Sekrety: `SecretString` (zerowany przy zwolnieniu, zredagowany w `Debug`) i trait `SecretStore`.

use std::fmt;

use zeroize::Zeroizing;

use crate::ids::SecretName;

/// Znacznik redakcji w `Debug`.
pub const REDACTED: &str = "***";

/// Wartość sekretu. Nie implementuje `Display`, `Serialize` ani `PartialEq`; `Debug` pokazuje `***`;
/// pamięć jest zerowana przy zwolnieniu (także kopie z `clone`).
#[derive(Clone)]
pub struct SecretString(Zeroizing<String>);

impl SecretString {
    /// Opakowuje wartość (bez modyfikacji).
    pub fn new(value: String) -> Self {
        Self(Zeroizing::new(value))
    }

    /// Wartość wklejona przez użytkownika: obcina białe znaki z brzegów.
    pub fn from_input(value: &str) -> Self {
        Self::new(value.trim().to_owned())
    }

    /// Jawny dostęp do wartości — tylko do przekazania adapterowi dostawcy lub magazynowi.
    pub fn expose_secret(&self) -> &str {
        &self.0
    }

    /// Czy pusty.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Porównanie w czasie niezależnym od pozycji pierwszej różnicy (dla równych długości).
    pub fn ct_eq(&self, other: &SecretString) -> bool {
        let (a, b) = (self.0.as_bytes(), other.0.as_bytes());
        a.len() == b.len() && a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
    }

    /// Format klucza API: niepusty, bez białych i sterujących znaków, ≤ 4096 bajtów.
    pub fn is_plausible_key(&self) -> bool {
        !self.0.is_empty()
            && self.0.len() <= 4096
            && !self.0.chars().any(|c| c.is_whitespace() || c.is_control())
    }

    /// Zastępuje wystąpienia sekretu w tekście znacznikiem `***` (obrona przed wyciekiem
    /// w komunikatach błędów zewnętrznych bibliotek).
    pub fn redact_in(&self, text: &str) -> String {
        if self.0.len() < 4 {
            return text.to_owned();
        }
        text.replace(self.0.as_str(), REDACTED)
    }
}

impl fmt::Debug for SecretString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "SecretString({REDACTED})")
    }
}

impl From<String> for SecretString {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

impl From<&str> for SecretString {
    fn from(value: &str) -> Self {
        Self::new(value.to_owned())
    }
}

/// Błędy magazynu sekretów. Komunikaty nigdy nie zawierają wartości sekretu.
#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq)]
pub enum SecretStoreError {
    /// Magazyn niedostępny (np. brak Credential Manager na tej platformie).
    #[error("magazyn sekretów niedostępny: {0}")]
    Unavailable(String),
    /// Wartość za długa dla magazynu.
    #[error("sekret za długi (limit {max} B)")]
    TooLong {
        /// Limit w bajtach.
        max: usize,
    },
    /// Wpis istnieje, ale ma nieoczekiwany format (np. nie UTF-8/UTF-16).
    #[error("wpis `{0}` ma nieoczekiwany format")]
    BadFormat(String),
    /// Inny błąd magazynu.
    #[error("błąd magazynu sekretów: {0}")]
    Backend(String),
}

/// Magazyn sekretów (produkcyjnie: Windows Credential Manager z prefiksem `Alfa/`).
/// Operacje są synchroniczne (API systemowe jest synchroniczne i szybkie).
pub trait SecretStore: Send + Sync {
    /// Zapisuje (nadpisuje) sekret.
    fn put(&self, name: &SecretName, value: &SecretString) -> Result<(), SecretStoreError>;

    /// Odczytuje sekret; `None`, gdy nie istnieje.
    fn get(&self, name: &SecretName) -> Result<Option<SecretString>, SecretStoreError>;

    /// Usuwa sekret; `true`, gdy istniał.
    fn delete(&self, name: &SecretName) -> Result<bool, SecretStoreError>;

    /// Nazwy wszystkich sekretów Alfy (bez prefiksu), posortowane.
    fn list(&self) -> Result<Vec<SecretName>, SecretStoreError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_is_redacted_and_eq_is_constant_shape() {
        let s = SecretString::from_input("  sk-live-123456  ");
        assert_eq!(s.expose_secret(), "sk-live-123456");
        assert_eq!(format!("{s:?}"), "SecretString(***)");
        assert!(s.ct_eq(&SecretString::from("sk-live-123456")));
        assert!(!s.ct_eq(&SecretString::from("sk-live-123457")));
        assert!(!s.ct_eq(&SecretString::from("sk")));
        assert!(s.is_plausible_key());
        assert!(!SecretString::from("a b").is_plausible_key());
        assert!(!SecretString::from("").is_plausible_key());
        assert_eq!(s.redact_in("401 for key sk-live-123456"), "401 for key ***");
        assert_eq!(SecretString::from("ab").redact_in("ab"), "ab");
    }
}
