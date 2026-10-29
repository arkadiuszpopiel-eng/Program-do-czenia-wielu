//! Polecenia zakładające lokalną maszynę (B.5, ADR 0019): okna systemu na ekranie hosta, podgląd
//! `asset:`, ścieżki bezwzględne hosta, okna/schowek/terminal hosta — i ich odpowiedniki dla klienta
//! zdalnego (istniejące polecenie, zadanie etapu D albo „tylko lokalnie” z powodem). Tabela jest danymi
//! (`data/local-only.json`), trafia do schematu łącza i do typów TS; brama zdalna (D.1) odmówi tych
//! poleceń klientowi spoza okna lokalnego. Pola DTO ze ścieżką hosta ([`LocalOnly::host_paths`]) brama
//! skróci albo zastąpi uchwytem — klient zdalny nie poznaje układu dysku hosta.

use std::collections::BTreeMap;
use std::sync::LazyLock;

use serde::Deserialize;

/// Tabela w postaci źródłowej (także dla schematu łącza).
pub const LOCAL_ONLY_JSON: &str = include_str!("../data/local-only.json");

/// Dlaczego polecenie wymaga lokalnej maszyny.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LocalReason {
    /// Okno systemu (wybór pliku, katalogu, „Zapisz jako”) na ekranie hosta.
    SystemDialog,
    /// Podgląd przez protokół `asset:` (plik hosta).
    AssetPreview,
    /// Ścieżka bezwzględna hosta w argumencie albo wyniku.
    HostPath,
    /// Okna, schowek, terminal, ustawienia albo restart na hoście.
    HostDesktop,
}

/// Odpowiednik dla klienta zdalnego.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(untagged)]
pub enum Remote {
    /// Istniejące polecenie, które robi to samo bez lokalnej maszyny.
    Command {
        /// Nazwa polecenia.
        command: String,
    },
    /// Odpowiednik zaplanowany w zadaniu etapu D.
    Planned {
        /// Zadanie (`D.1`, `D.5`, `D.6`).
        planned: String,
        /// Sposób.
        how: String,
    },
    /// Tylko lokalnie.
    None {
        /// Powód.
        none: String,
    },
}

/// Polecenie oznaczone `local_only`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct LocalOnlyCommand {
    /// Powód.
    pub reason: LocalReason,
    /// Odpowiednik dla klienta zdalnego.
    pub remote: Remote,
}

/// Cała tabela.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
pub struct LocalOnly {
    /// Polecenie → powód i odpowiednik.
    pub commands: BTreeMap<String, LocalOnlyCommand>,
    /// Pola DTO ze ścieżką hosta (`Typ.pole`).
    pub host_paths: Vec<String>,
}

static TABLE: LazyLock<LocalOnly> =
    LazyLock::new(|| serde_json::from_str(LOCAL_ONLY_JSON).unwrap_or_default());

/// Tabela poleceń `local_only` (pusta tylko przy uszkodzonym pliku — pilnuje tego test).
pub fn local_only() -> &'static LocalOnly {
    &TABLE
}

/// Czy polecenie wymaga lokalnej maszyny (klient zdalny go nie dostanie).
pub fn is_local_only(command: &str) -> bool {
    TABLE.commands.contains_key(command)
}
