//! Argumenty i wyniki narzędzi plikowych (źródło JSON Schema dla modelu i UI).

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// `fs_list`: lista katalogu.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ListArgs {
    /// Katalog (bezwzględny albo względny wobec katalogu roboczego; bez `..`).
    pub path: String,
    /// Głębokość rekursji 0–3 (0 = tylko bezpośrednie wpisy).
    #[serde(default)]
    pub depth: Option<u8>,
    /// Maksymalna liczba wpisów (domyślnie z konfiguracji).
    #[serde(default)]
    pub limit: Option<u32>,
}

/// `fs_read`: odczyt pliku tekstowego (zakresem bajtów).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ReadArgs {
    /// Plik.
    pub path: String,
    /// Przesunięcie w bajtach (domyślnie 0).
    #[serde(default)]
    pub offset: Option<u64>,
    /// Maksymalna liczba bajtów (domyślnie i najwyżej limit z konfiguracji).
    #[serde(default)]
    pub max_bytes: Option<u32>,
}

/// `fs_stat`, `fs_delete`, `fs_delete_permanent`, `fs_mkdir`: jedna ścieżka.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PathArgs {
    /// Ścieżka.
    pub path: String,
}

/// `fs_search`: wyszukiwanie po nazwie (glob `*`, `?`) i opcjonalnie po treści.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SearchArgs {
    /// Katalog startowy.
    pub root: String,
    /// Wzorzec nazwy pliku, np. `*.pdf` (bez rozróżniania wielkości liter).
    pub pattern: String,
    /// Fragment tekstu, który musi wystąpić w pliku (opcjonalnie).
    #[serde(default)]
    pub content: Option<String>,
    /// Maksymalna liczba wyników.
    #[serde(default)]
    pub max_results: Option<u32>,
    /// Maksymalna głębokość katalogów.
    #[serde(default)]
    pub max_depth: Option<u8>,
}

/// Tryb zapisu.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum WriteMode {
    /// Nowy plik — błąd, gdy istnieje (domyślnie, najbezpieczniej).
    #[default]
    Create,
    /// Utwórz albo zastąp (poprzednia treść w dzienniku cofania).
    Overwrite,
    /// Dopisz na końcu (utwórz, gdy brak).
    Append,
}

/// `fs_write`: zapis tekstu (atomowy, z pre-image w dzienniku cofania).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WriteArgs {
    /// Plik.
    pub path: String,
    /// Treść (UTF-8).
    pub content: String,
    /// Tryb (domyślnie `create`).
    #[serde(default)]
    pub mode: WriteMode,
}

/// `fs_move`, `fs_copy`: źródło i cel (cel nie może istnieć).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FromToArgs {
    /// Źródło.
    pub from: String,
    /// Cel.
    pub to: String,
}

/// `fs_rename`: nowa nazwa w tym samym katalogu.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RenameArgs {
    /// Plik.
    pub path: String,
    /// Nowa nazwa (bez separatorów ścieżki).
    pub new_name: String,
}

/// Wpis katalogu.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Entry {
    /// Ścieżka.
    pub path: String,
    /// Katalog.
    pub is_dir: bool,
    /// Rozmiar (B; 0 dla katalogu).
    pub size: u64,
}

/// Wynik `fs_list`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ListOutput {
    /// Katalog.
    pub path: String,
    /// Wpisy.
    pub entries: Vec<Entry>,
    /// Obcięto limitem.
    pub truncated: bool,
}

/// Wynik `fs_read`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ReadOutput {
    /// Plik.
    pub path: String,
    /// Treść fragmentu (zredagowana z sekretów); pusta dla plików binarnych.
    pub content: String,
    /// Rozmiar pliku (B).
    pub bytes_total: u64,
    /// Przesunięcie fragmentu.
    pub offset: u64,
    /// Fragment krótszy niż reszta pliku.
    pub truncated: bool,
    /// Plik binarny (treść pominięta).
    pub binary: bool,
}

/// Wynik `fs_stat`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct StatOutput {
    /// Ścieżka.
    pub path: String,
    /// Istnieje.
    pub exists: bool,
    /// Katalog.
    pub is_dir: bool,
    /// Rozmiar (B).
    pub size: u64,
}

/// Trafienie wyszukiwania.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SearchHit {
    /// Plik.
    pub path: String,
    /// Pierwsza pasująca linia (gdy szukano treści; zredagowana, ≤ 200 znaków).
    pub line: Option<String>,
}

/// Wynik `fs_search`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SearchOutput {
    /// Katalog startowy.
    pub root: String,
    /// Trafienia.
    pub hits: Vec<SearchHit>,
    /// Liczba odwiedzonych wpisów.
    pub visited: u32,
    /// Przerwano limitem (wyników, odwiedzin albo głębokości).
    pub truncated: bool,
}

/// Wynik operacji zmieniającej stan.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct MutationOutput {
    /// Ścieżki (cel; dla przeniesienia: źródło i cel).
    pub paths: Vec<String>,
    /// Zapisane bajty (zapis).
    pub bytes: u64,
    /// Identyfikator kroku „Cofnij” w dzienniku.
    pub undo_step: Option<u64>,
    /// Czy obiekt istnieje po operacji (mkdir).
    pub exists: bool,
}
