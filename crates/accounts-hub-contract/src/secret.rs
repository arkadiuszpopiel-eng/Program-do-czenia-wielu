//! Sekrety: `SecretString` (zerowany przy zwolnieniu, zredagowany w `Debug`) i trait `SecretStore`.

use std::fmt;

use zeroize::Zeroizing;

use crate::ids::SecretName;

/// `text` bez początku `prefix` (bez rozróżniania wielkości liter ASCII).
fn strip_prefix_ignore_case<'a>(text: &'a str, prefix: &str) -> Option<&'a str> {
    let head = text.get(..prefix.len())?;
    head.eq_ignore_ascii_case(prefix)
        .then(|| text.get(prefix.len()..))
        .flatten()
}

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

    /// Wartość wklejona przez użytkownika. Schowek często niesie więcej niż sam klucz: cudzysłowy,
    /// `OPENAI_API_KEY=…` z pliku `.env`, `set NAZWA=…` albo `$env:NAZWA=…` z konsoli, nagłówek
    /// `Authorization: Bearer …`, przecinek z pliku JSON, znak nowej linii z zawiniętego wiersza.
    /// Zdejmujemy tylko to, co da się rozpoznać jednoznacznie; reszta (np. zdanie ze spacjami)
    /// zostaje i nie przejdzie [`Self::is_plausible_key`] — nie sklejamy przypadkowego tekstu w „klucz”.
    pub fn from_input(value: &str) -> Self {
        let invisible = |c: char| {
            matches!(c,
                '\u{0085}' | '\u{00A0}' | '\u{00AD}' | '\u{200B}'..='\u{200F}' | '\u{202A}'..='\u{202E}'
                | '\u{2060}'..='\u{2069}' | '\u{FEFF}')
        };
        let visible: String = value.chars().filter(|c| !invisible(*c)).collect();
        let quotes = |c: char| {
            matches!(
                c,
                '"' | '\'' | '`' | '„' | '”' | '“' | '’' | '‘' | '«' | '»'
            )
        };
        let mut text = visible.trim();
        // Kilka warstw naraz (np. `export NAZWA="Bearer klucz",`) — powtarzamy do skutku.
        for _ in 0..6 {
            let before = text;
            text = text.trim_end_matches([',', ';']).trim();
            text = text.trim_matches(quotes).trim();
            for prefix in ["export ", "set ", "setx ", "$env:"] {
                if let Some(rest) = strip_prefix_ignore_case(text, prefix) {
                    text = rest.trim_start();
                }
            }
            if let Some(rest) = strip_prefix_ignore_case(text, "authorization:") {
                text = rest.trim_start();
            }
            if let Some(rest) = strip_prefix_ignore_case(text, "bearer ") {
                text = rest.trim_start();
            }
            // `NAZWA_KEY=wartość`, `nazwa_token: wartość`, `setx NAZWA wartość`.
            if let Some(at) = text.find(['=', ':', ' ']) {
                let (name, rest) = text.split_at(at);
                let upper = name.trim().trim_matches(quotes).to_ascii_uppercase();
                let after = rest.get(1..).unwrap_or_default();
                let value = after.trim().trim_matches(quotes);
                let is_variable = !upper.is_empty()
                    && upper
                        .chars()
                        .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_')
                    && (upper.ends_with("KEY") || upper.ends_with("TOKEN"))
                    // `apikey:tajne` bywa całym kluczem — dwukropek zdejmujemy tylko po nazwie
                    // z podkreśleniem albo gdy po nim jest odstęp lub cudzysłów (`"apiKey": "…"`).
                    && (!rest.starts_with(':')
                        || upper.contains('_')
                        || after.starts_with([' ', '"', '\'']))
                    // Reszta ma wyglądać jak wartość: `…KEY==` to koniec klucza base64.
                    && value.len() >= 8
                    && value.chars().any(|c| c != '=');
                if is_variable {
                    text = after.trim();
                }
            }
            if text == before {
                break;
            }
        }
        // Zawinięty wiersz sklejamy. Dwa osobne wpisy (dwie linie pliku `.env`, dwa razy ten sam
        // klucz) zostają rozdzielone i nie przejdą `is_plausible_key` — sklejenie wysłałoby
        // klucz jednego dostawcy do drugiego.
        let parts: Vec<&str> = text
            .split(['\r', '\n', '\t'])
            .map(str::trim)
            .filter(|part| !part.is_empty())
            .collect();
        let separate = parts.iter().enumerate().skip(1).any(|(at, part)| {
            part.contains([' ', ':'])
                || part.trim_end_matches('=').contains('=')
                || parts.iter().take(at).any(|earlier| earlier == part)
        });
        Self::new(if separate {
            parts.join("\n")
        } else {
            parts.concat()
        })
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

    /// Format klucza API: niepusty, same drukowalne znaki ASCII (bez spacji), ≤ 4096 bajtów.
    /// Klucze dostawców trafiają do nagłówka HTTP — inne znaki i tak by tam nie przeszły.
    pub fn is_plausible_key(&self) -> bool {
        !self.0.is_empty() && self.0.len() <= 4096 && self.0.chars().all(|c| c.is_ascii_graphic())
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
        // Tabela wspólna z interfejsem (`apps/desktop/ui/src/lib/logic/api-key.ts` czyta ten sam plik):
        // co da się zdjąć jednoznacznie — znika; czego nie — zostaje i nie jest „kluczem”.
        let table: serde_json::Value =
            serde_json::from_str(include_str!("../tests/fixtures/key-inputs.json")).unwrap();
        let key = table["key"].as_str().unwrap();
        for pasted in table["cleaned"].as_array().unwrap() {
            let pasted = pasted.as_str().unwrap().replace("{key}", key);
            let cleaned = SecretString::from_input(&pasted);
            assert_eq!(cleaned.expose_secret(), key, "{pasted:?}");
            assert!(cleaned.is_plausible_key());
        }
        for pasted in table["rejected"].as_array().unwrap() {
            let pasted = pasted.as_str().unwrap().replace("{key}", key);
            assert!(
                !SecretString::from_input(&pasted).is_plausible_key(),
                "{pasted:?} nie jest kluczem"
            );
        }
        for kept in table["unchanged"].as_array().unwrap() {
            let kept = kept.as_str().unwrap();
            assert_eq!(SecretString::from_input(kept).expose_secret(), kept);
        }
    }
}
