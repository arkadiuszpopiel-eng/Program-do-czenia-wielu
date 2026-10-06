//! Błędy modułu `transfer` (serializowalne — trafiają do UI przez IPC). Komunikaty nigdy nie
//! zawierają wartości sekretów ani haseł.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::paths::PathError;

/// Błędy importu/eksportu.
#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "error", rename_all = "snake_case")]
pub enum TransferError {
    /// Plik nie jest paczką `.alfa` albo jest uszkodzony/obcięty.
    #[error("uszkodzona albo obcięta paczka: {reason}")]
    Corrupt {
        /// Opis problemu.
        reason: String,
    },
    /// Suma kontrolna lub rozmiar wpisu niezgodne z manifestem.
    #[error("niezgodna suma kontrolna wpisu `{path}`")]
    Checksum {
        /// Ścieżka wpisu.
        path: String,
    },
    /// Niedozwolona ścieżka wpisu (zip-slip, ścieżka bezwzględna, `C:\`, UNC…).
    #[error("niedozwolona ścieżka wpisu `{path}`: {reason}")]
    UnsafePath {
        /// Ścieżka (tak, jak zapisano ją w paczce).
        path: String,
        /// Powód odrzucenia.
        reason: PathError,
    },
    /// Przekroczony limit (rozmiar po rozpakowaniu, liczba wpisów, rozmiar wpisu) — zip-bomb.
    #[error("przekroczony limit paczki: {what} ({actual} > {max})")]
    LimitExceeded {
        /// Czego dotyczy limit.
        what: String,
        /// Wartość rzeczywista (albo zadeklarowana).
        actual: u64,
        /// Limit.
        max: u64,
    },
    /// Paczka nowsza niż obsługiwana — trzeba zaktualizować Alfę.
    #[error(
        "paczka ma schemat {found}, a ta wersja Alfy obsługuje najwyżej {supported} — zaktualizuj Alfę"
    )]
    NewerSchema {
        /// Wersja schematu paczki.
        found: String,
        /// Najwyższa obsługiwana wersja.
        supported: String,
    },
    /// Paczka ze starszym „major” schematu, dla którego ta wersja nie ma upcastera (fala 5, m-06)
    /// — to **nie** jest paczka z nowszej Alfy, więc komunikat nie radzi aktualizacji.
    #[error(
        "paczka ma schemat {found}, starszy niż najstarszy obsługiwany ({oldest}) — ta wersja Alfy \
         nie ma dla niego migracji (to nie jest paczka z nowszej wersji)"
    )]
    OlderSchema {
        /// Wersja schematu paczki.
        found: String,
        /// Najstarsza wersja przyjmowana (bezpośrednio albo łańcuchem upcasterów).
        oldest: String,
    },
    /// Paczka zaszyfrowana, a nie podano hasła.
    #[error("paczka jest zaszyfrowana — podaj hasło")]
    PasswordRequired,
    /// Złe hasło albo zmodyfikowany szyfrogram (AEAD nie rozróżnia tych przypadków).
    #[error("złe hasło albo uszkodzony szyfrogram")]
    WrongPassword,
    /// Hasło za słabe (za krótkie).
    #[error("hasło musi mieć co najmniej {min} znaków")]
    WeakPassword {
        /// Minimalna długość.
        min: usize,
    },
    /// Operacja wymaga szyfrowania (sesje prywatne).
    #[error("{what} wymaga paczki zaszyfrowanej hasłem")]
    EncryptionRequired {
        /// Czego dotyczy wymóg.
        what: String,
    },
    /// Paczka sekretów (eksport ze starszej wersji Alfy) — sekretów nie importuje się z `.alfa`
    /// (AGENTS.md: tylko Credential Manager; CX-a).
    #[error(
        "to paczka sekretów ze starszej wersji Alfy — sekretów nie importuje się z plików .alfa; \
         dodaj klucze ponownie w Ustawienia → Konta"
    )]
    SecretsNotAllowed,
    /// W treści przeznaczonej do zwykłej paczki wykryto sekret (ostatnia linia obrony).
    #[error("wpis `{path}` zawiera sekret — eksport przerwany")]
    SecretDetected {
        /// Ścieżka wpisu.
        path: String,
    },
    /// Nieprawidłowe dane (manifest, rekord sesji, dokument).
    #[error("nieprawidłowe dane w `{path}`: {reason}")]
    Invalid {
        /// Ścieżka wpisu albo nazwa elementu.
        path: String,
        /// Opis problemu.
        reason: String,
    },
    /// Element nie istnieje (snapshot, sesja wskazana w zakresie…).
    #[error("nie znaleziono: {what}")]
    NotFound {
        /// Czego szukano.
        what: String,
    },
    /// Brak portu (np. magazynu sesji) potrzebnego dla elementu paczki.
    #[error("brak obsługi kategorii `{category}` w tej instalacji")]
    Unsupported {
        /// Kategoria.
        category: String,
    },
    /// Operacja anulowana przez użytkownika.
    #[error("anulowano")]
    Cancelled,
    /// Błąd portu (sesje, dokumenty, magazyn sekretów).
    #[error("{port}: {reason}")]
    Port {
        /// Nazwa portu.
        port: String,
        /// Opis (bez treści sekretów).
        reason: String,
    },
    /// Błąd wejścia/wyjścia.
    #[error("we/wy: {reason}")]
    Io {
        /// Opis problemu.
        reason: String,
    },
}

impl TransferError {
    /// Skrót: błąd we/wy.
    pub fn io(err: impl std::fmt::Display) -> Self {
        TransferError::Io {
            reason: err.to_string(),
        }
    }

    /// Skrót: uszkodzona paczka.
    pub fn corrupt(reason: impl Into<String>) -> Self {
        TransferError::Corrupt {
            reason: reason.into(),
        }
    }

    /// Skrót: nieprawidłowe dane w elemencie `path`.
    pub fn invalid(path: impl Into<String>, reason: impl std::fmt::Display) -> Self {
        TransferError::Invalid {
            path: path.into(),
            reason: reason.to_string(),
        }
    }

    /// Skrót: błąd portu.
    pub fn port(port: &str, err: impl std::fmt::Display) -> Self {
        TransferError::Port {
            port: port.to_owned(),
            reason: err.to_string(),
        }
    }
}

impl From<std::io::Error> for TransferError {
    fn from(err: std::io::Error) -> Self {
        TransferError::io(err)
    }
}

impl From<sessions_contract::SessionError> for TransferError {
    fn from(err: sessions_contract::SessionError) -> Self {
        TransferError::port("sesje", err)
    }
}

impl From<accounts_hub_contract::SecretStoreError> for TransferError {
    fn from(err: accounts_hub_contract::SecretStoreError) -> Self {
        TransferError::port("magazyn sekretów", err)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn errors_serialize_tagged_and_read_polish() {
        let json = serde_json::to_value(TransferError::WrongPassword).unwrap();
        assert_eq!(json, serde_json::json!({"error": "wrong_password"}));
        let e = TransferError::NewerSchema {
            found: "2.0.0".into(),
            supported: "1.0.0".into(),
        };
        assert!(e.to_string().contains("zaktualizuj Alfę"));
        let back: TransferError =
            serde_json::from_value(serde_json::to_value(&e).unwrap()).unwrap();
        assert_eq!(back, e);
    }
}
