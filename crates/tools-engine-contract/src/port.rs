//! Port silnika: instalacja, cykl życia serwera, wywołania, zrzuty, podgląd i dokumentacja.
//! Implementacja (`-impl`) uruchamia `engine-server.exe`; atrapa (`-fake`) trzyma świat w pamięci.

use std::path::PathBuf;

use async_trait::async_trait;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::EngineMethod;

/// Błąd silnika (komunikaty po polsku, dla modelu i UI).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum EngineError {
    /// Nie wskazano katalogu silnika w Ustawieniach.
    #[error(
        "nie wskazano katalogu silnika (Ustawienia → Zaawansowane → Katalog silnika gier NextGO.Gray)"
    )]
    NotConfigured,
    /// Brak zbudowanego programu.
    #[error("brak programu silnika: {0} — zbuduj silnik (cargo build --release) w jego katalogu")]
    NotInstalled(String),
    /// Serwer nie działa.
    #[error("serwer silnika nie działa — uruchom go narzędziem `engine_start`")]
    NotRunning,
    /// Serwer już działa.
    #[error(
        "serwer silnika już działa ({0}) — zatrzymaj go `engine_stop`, jeśli chcesz inną scenę"
    )]
    AlreadyRunning(String),
    /// Uruchomienie procesu nie powiodło się.
    #[error("uruchomienie silnika: {0}")]
    Spawn(String),
    /// Łącze z serwerem.
    #[error("połączenie z silnikiem: {0}")]
    Transport(String),
    /// Odpowiedź spoza protokołu.
    #[error("protokół silnika: {0}")]
    Protocol(String),
    /// Brak odpowiedzi w czasie.
    #[error("silnik nie odpowiedział w {0} s")]
    Timeout(u64),
    /// Wiadomość ponad limit serwera.
    #[error("wiadomość ma {bytes} B, limit silnika to {limit} B — podziel transakcję")]
    TooLarge {
        /// Rozmiar.
        bytes: usize,
        /// Limit.
        limit: usize,
    },
    /// Błąd zgłoszony przez silnik (JSON-RPC).
    #[error("silnik odrzucił żądanie ({kind}): {message}")]
    Rpc {
        /// Kod JSON-RPC.
        code: i64,
        /// `data.code` silnika (`revision_conflict`, `invalid_input`, …).
        kind: String,
        /// Komunikat silnika.
        message: String,
    },
    /// Plik (dokumentacja, zrzut, podgląd).
    #[error("plik: {0}")]
    Io(String),
}

/// Scena startowa.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Scene {
    /// Pusty świat.
    #[default]
    Empty,
    /// Scena pokazowa (`--demo`).
    Demo,
    /// Scena przemysłowa (`--industrial`).
    Industrial,
}

/// Opcje uruchomienia serwera.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StartOptions {
    /// Scena startowa.
    pub scene: Scene,
    /// Zapis świata do wczytania (`--load`, `.json` albo bincode; zamiast `scene`).
    pub load: Option<PathBuf>,
    /// Czas stoi — tylko `engine_step` go przesuwa (powtarzalność).
    pub paused: bool,
    /// Okno podglądu GPU; zrzut każdej nowej rewizji do katalogu zrzutów (wymusza `paused`).
    pub window: bool,
    /// Wyjście dźwięku.
    pub audio: bool,
}

/// Opcje jednorazowego podglądu (osobny proces: scena → N klatek → PNG + telemetria).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PreviewOptions {
    /// Scena.
    pub scene: Scene,
    /// Zapis świata (zamiast `scene`).
    pub load: Option<PathBuf>,
    /// Klatka, po której powstaje zrzut.
    pub frames: u32,
    /// Szerokość (px).
    pub width: u32,
    /// Wysokość (px).
    pub height: u32,
}

/// Stan serwera.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct EngineState {
    /// Działa.
    pub running: bool,
    /// Adres WebSocket (tylko lokalny).
    pub url: Option<String>,
    /// `headless` albo `gpu_preview`.
    pub mode: Option<String>,
    /// Czas stoi.
    pub paused: bool,
    /// Okno podglądu ze zrzutami rewizji.
    pub window: bool,
    /// Numer procesu.
    pub pid: Option<u32>,
    /// Scena albo wczytany plik.
    pub scene: Option<String>,
    /// Kod wyjścia, gdy proces zakończył się sam.
    pub exit_code: Option<i32>,
}

/// Instalacja silnika (odczyt katalogu, bez uruchamiania).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Installation {
    /// Katalog silnika z Ustawień.
    pub root: Option<PathBuf>,
    /// `target/release/engine-server.exe`, gdy istnieje.
    pub server: Option<PathBuf>,
    /// `target/release/engine-mcp-bridge.exe`, gdy istnieje.
    pub bridge: Option<PathBuf>,
    /// Wersja z `[workspace.package]` w `Cargo.toml`.
    pub version: Option<String>,
    /// Stan bramek z `artifacts/verification.json` (`passed`, …).
    pub verification: Option<String>,
    /// Czas sprawdzenia (UTC) z tego samego pliku.
    pub verified_utc: Option<String>,
}

/// Zrzut ekranu silnika.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Capture {
    /// Plik PNG (w katalogu danych Alfy, nigdy w katalogu silnika).
    pub png: PathBuf,
    /// Rewizja świata.
    pub revision: Option<u64>,
    /// Telemetria renderera (`preview.json` / `revision-N.json`).
    pub telemetry: Value,
}

/// Plik dokumentacji silnika.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct DocEntry {
    /// Nazwa względem katalogu silnika (`docs/API_ETAP_4.md`).
    pub name: String,
    /// Rozmiar w bajtach.
    pub bytes: u64,
    /// Pierwszy nagłówek.
    pub title: String,
}

/// Port silnika.
#[async_trait]
pub trait EnginePort: Send + Sync {
    /// Instalacja (katalog, programy, wersja, ostatnie sprawdzenie).
    fn installation(&self) -> Installation;

    /// Stan serwera.
    fn state(&self) -> EngineState;

    /// Uruchamia serwer i łączy się z nim.
    async fn start(&self, options: StartOptions) -> Result<EngineState, EngineError>;

    /// Zatrzymuje serwer; `false`, gdy nie działał.
    async fn stop(&self) -> Result<bool, EngineError>;

    /// Wywołanie JSON-RPC (wynik `result` serwera).
    async fn call(&self, method: EngineMethod, params: Value) -> Result<Value, EngineError>;

    /// Ostatni zrzut rewizji z okna podglądu (`None` — brak zrzutu albo okno wyłączone).
    async fn capture(&self) -> Result<Option<Capture>, EngineError>;

    /// Jednorazowy podgląd w osobnym procesie.
    async fn preview(&self, options: PreviewOptions) -> Result<Capture, EngineError>;

    /// Lista dokumentacji (`README.md`, `SPEC*.md`, `AGENTS.md`, `docs/*.md`).
    async fn docs(&self) -> Result<Vec<DocEntry>, EngineError>;

    /// Treść pliku dokumentacji z listy (`name` jak w [`DocEntry`]).
    async fn read_doc(&self, name: &str) -> Result<String, EngineError>;

    /// Kill-switch: kończy serwer i podgląd bez czekania; liczba zakończonych procesów.
    fn kill_all(&self) -> usize;
}
