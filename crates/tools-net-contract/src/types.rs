//! Argumenty (zamknięte schematy) i wyniki narzędzi `tools-net`.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::port::{HttpMethod, SearchHit};

/// `net_fetch`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FetchArgs {
    /// Adres `https://…` hosta publicznego.
    pub url: String,
    /// Metoda (`get` domyślnie, `head` — tylko nagłówki).
    #[serde(default)]
    pub method: Option<HttpMethod>,
    /// Limit znaków tekstu w wyniku (100–100 000).
    #[serde(default)]
    pub max_chars: Option<u32>,
}

/// `net_download`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DownloadArgs {
    /// Adres `https://…` pliku.
    pub url: String,
    /// Proponowana nazwa pliku (zostanie oczyszczona); domyślnie z serwera albo adresu.
    #[serde(default)]
    pub file_name: Option<String>,
}

/// `net_search`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SearchArgs {
    /// Zapytanie.
    pub query: String,
    /// Najwięcej wyników (1–20).
    #[serde(default)]
    pub max_results: Option<u32>,
}

/// Wynik `net_fetch`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct FetchOut {
    /// Adres żądany.
    pub url: String,
    /// Adres końcowy (po przekierowaniach).
    pub final_url: String,
    /// Status HTTP.
    pub status: u16,
    /// Typ treści.
    pub content_type: Option<String>,
    /// Odczytane bajty.
    pub bytes: u64,
    /// Tekst (niezaufany, zredagowany; tylko dla typów tekstowych).
    pub text: Option<String>,
    /// Treść albo tekst obcięte limitem.
    pub truncated: bool,
    /// Kolejne adresy przekierowań.
    pub redirects: Vec<String>,
}

/// Wynik `net_download`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct DownloadOut {
    /// Ścieżka w kwarantannie.
    pub path: String,
    /// Rozmiar.
    pub bytes: u64,
    /// SHA-256 (hex).
    pub sha256: String,
    /// Typ treści (niezaufany).
    pub content_type: Option<String>,
    /// Adres końcowy.
    pub final_url: String,
    /// Rozszerzenie uruchamialne (`.exe`, `.ps1`, `.lnk`…) — nie uruchamiaj.
    pub executable: bool,
}

/// Wynik `net_search`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SearchOut {
    /// Wyniki (niezaufane).
    pub results: Vec<SearchHit>,
}
