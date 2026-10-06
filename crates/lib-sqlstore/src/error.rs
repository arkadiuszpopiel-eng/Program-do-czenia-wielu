//! Błędy warstwy danych.

/// Błędy `lib-sqlstore`.
#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    /// Błąd SQLite/SQLCipher/sqlite-vec.
    #[error("sqlite: {0}")]
    Sqlite(#[from] rusqlite::Error),
    /// Plik nie jest bazą albo klucz jest zły (SQLCipher nie odróżnia tych przypadków).
    #[error("plik nie jest bazą danych albo klucz jest nieprawidłowy")]
    NotDatabaseOrWrongKey,
    /// `sqlite3_auto_extension` zwróciło kod błędu przy rejestracji sqlite-vec.
    #[error("rejestracja sqlite-vec nie powiodła się (kod {0})")]
    VecRegistration(i32),
    /// Błąd wejścia/wyjścia (katalogi, usuwanie plików).
    #[error("we/wy: {0}")]
    Io(#[from] std::io::Error),
    /// Generator losowy systemu nie zwrócił klucza.
    #[error("brak losowości systemowej: {0}")]
    Random(String),
    /// Klucz w złym formacie (np. hex o złej długości).
    #[error("nieprawidłowy klucz: {0}")]
    InvalidKey(String),
    /// Lista migracji jest niepoprawna (pusta wersja, duplikat, zła kolejność).
    #[error("nieprawidłowa lista migracji `{namespace}`: {reason}")]
    InvalidMigrations {
        /// Przestrzeń nazw migracji (moduł).
        namespace: String,
        /// Opis problemu.
        reason: String,
    },
    /// Baza zawiera migrację nieznaną kodowi, a nie nowszą od znanych (luka albo rozwidlenie
    /// historii migracji). Bazę z nowszej wersji programu `migrate` przyjmuje bez zmian.
    #[error("baza ma nieznaną migrację `{namespace}/{version}` spoza historii tej wersji programu")]
    UnknownMigration {
        /// Przestrzeń nazw.
        namespace: String,
        /// Wersja zapisana w bazie.
        version: String,
    },
    /// Oczekująca migracja jest starsza niż już zastosowana (wstawiona „w środek”).
    #[error("migracja `{namespace}/{version}` jest starsza niż zastosowana `{applied}`")]
    MigrationOutOfOrder {
        /// Przestrzeń nazw.
        namespace: String,
        /// Oczekująca wersja.
        version: String,
        /// Najnowsza zastosowana wersja.
        applied: String,
    },
    /// Połączenie jest tylko do odczytu (baza z nowszej wersji programu), a przestrzeń nazw
    /// wymaga migracji — nic nie zapisano.
    #[error(
        "baza z nowszej wersji Alfy jest tylko do odczytu — nie można zastosować migracji \
         `{namespace}/{version}` (zaktualizuj Alfę)"
    )]
    ReadOnly {
        /// Przestrzeń nazw.
        namespace: String,
        /// Wersja, której nie zastosowano.
        version: String,
    },
    /// Migracja nie powiodła się (wycofana w całości).
    #[error("migracja `{namespace}/{version}` nie powiodła się: {source}")]
    MigrationFailed {
        /// Przestrzeń nazw.
        namespace: String,
        /// Wersja.
        version: String,
        /// Przyczyna.
        source: rusqlite::Error,
    },
}

impl StoreError {
    /// Mapuje błąd otwarcia: `SQLITE_NOTADB` → [`StoreError::NotDatabaseOrWrongKey`].
    pub(crate) fn from_open(err: rusqlite::Error) -> Self {
        match err.sqlite_error_code() {
            Some(rusqlite::ErrorCode::NotADatabase) => StoreError::NotDatabaseOrWrongKey,
            _ => StoreError::Sqlite(err),
        }
    }
}
