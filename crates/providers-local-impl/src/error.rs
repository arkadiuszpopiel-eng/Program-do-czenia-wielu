//! Błędy i zdarzenia `local.*` dostawcy lokalnego.

use providers_contract::{ProviderError, ProviderErrorKind};
use serde::Serialize;

/// Błąd dostawcy lokalnego.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum LocalError {
    /// Niepoprawny manifest modeli.
    #[error("manifest modeli: {0}")]
    Manifest(String),
    /// Nieznany model.
    #[error("nieznany model lokalny `{0}`")]
    UnknownModel(String),
    /// Model niepobrany (albo bez zapisanego hasha).
    #[error("model `{0}` nie jest pobrany — pobierz go w Ustawieniach → Modele")]
    NotInstalled(String),
    /// Brak pliku `llama-server` dla backendu.
    #[error("brak serwera llama.cpp dla backendu {0}")]
    NoServerBinary(String),
    /// Uruchomienie procesu nie powiodło się.
    #[error("uruchomienie llama-server: {0}")]
    Spawn(String),
    /// Serwer nie odpowiedział zdrowiem w czasie startu albo zakończył się.
    #[error("llama-server nie wystartował: {0}")]
    Startup(String),
    /// Zbyt wiele restartów w oknie czasu.
    #[error("llama-server niestabilny: {0} awarii w oknie — wstrzymano automatyczny restart")]
    Unstable(u32),
    /// Zarządca rezydencji odmówił.
    #[error("brak pamięci na model: {0}")]
    Residency(String),
    /// Błąd pobierania.
    #[error("pobieranie: {0}")]
    Download(String),
    /// Niezgodny SHA-256 pobranego pliku.
    #[error("niezgodny SHA-256 pliku `{file}`: oczekiwano {expected}, jest {actual}")]
    HashMismatch {
        /// Plik.
        file: String,
        /// Oczekiwany hash.
        expected: String,
        /// Obliczony hash.
        actual: String,
    },
    /// Anulowano.
    #[error("anulowano")]
    Cancelled,
    /// Błąd I/O.
    #[error("I/O: {0}")]
    Io(String),
}

impl LocalError {
    /// Błąd dostawcy dla strumienia (Router przełączy się na inny cel).
    pub fn to_provider_error(&self) -> ProviderError {
        let kind = match self {
            Self::UnknownModel(_) => ProviderErrorKind::InvalidRequest,
            Self::Startup(_) | Self::Spawn(_) | Self::Unstable(_) => {
                ProviderErrorKind::Server { status: 503 }
            }
            Self::Cancelled => ProviderErrorKind::Network,
            _ => ProviderErrorKind::Unsupported,
        };
        ProviderError::new(kind, self.to_string())
    }
}

impl From<std::io::Error> for LocalError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e.to_string())
    }
}

/// Postęp pobierania modelu.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct DownloadProgress {
    /// Pobrane bajty (z wznowieniem).
    pub bytes: u64,
    /// Rozmiar całkowity, jeśli znany.
    pub total: Option<u64>,
    /// Czy to wznowienie (HTTP Range).
    pub resumed: bool,
}

/// Pobieranie rozpoczęte/postęp.
pub const EVENT_DOWNLOAD_PROGRESS: &str = "local.model.download.progress";
/// Pobieranie zakończone (hash zgodny albo zapisany przy pierwszym pobraniu).
pub const EVENT_DOWNLOAD_FINISHED: &str = "local.model.download.finished";
/// Pobieranie nieudane.
pub const EVENT_DOWNLOAD_FAILED: &str = "local.model.download.failed";
/// Model załadowany (sidecar zdrowy).
pub const EVENT_LOADED: &str = "local.model.loaded";
/// Model wyładowany (bezczynność, eksmisja, zatrzymanie).
pub const EVENT_UNLOADED: &str = "local.model.unloaded";
/// Fallback backendu (GPU → CPU).
pub const EVENT_BACKEND_FALLBACK: &str = "local.backend.fallback";
/// Awaria sidecara.
pub const EVENT_SIDECAR_CRASHED: &str = "local.sidecar.crashed";

/// Zdarzenie modułu (ładunek bez treści rozmowy i bez klucza API sidecara).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum LocalEvent {
    /// Postęp pobierania.
    DownloadProgress {
        /// Model.
        model: String,
        /// Postęp.
        progress: DownloadProgress,
    },
    /// Pobieranie zakończone.
    DownloadFinished {
        /// Model.
        model: String,
        /// SHA-256 pliku.
        sha256: String,
        /// Czy hash pochodził z manifestu (inaczej zapisany przy pierwszym pobraniu).
        verified: bool,
    },
    /// Pobieranie nieudane.
    DownloadFailed {
        /// Model.
        model: String,
        /// Opis.
        error: String,
    },
    /// Model załadowany.
    Loaded {
        /// Model.
        model: String,
        /// Warstwy na GPU.
        gpu_layers: u32,
        /// Backend (`vulkan`/`cuda`/`cpu`).
        backend: String,
        /// Czas startu (ms).
        startup_ms: u64,
    },
    /// Model wyładowany.
    Unloaded {
        /// Model.
        model: String,
        /// Powód.
        reason: String,
    },
    /// Fallback backendu.
    BackendFallback {
        /// Z.
        from: String,
        /// Na.
        to: String,
        /// Powód.
        reason: String,
    },
    /// Awaria sidecara.
    SidecarCrashed {
        /// Model.
        model: String,
        /// Kod wyjścia, jeśli znany.
        exit_code: Option<i32>,
    },
}

impl LocalEvent {
    /// Nazwa zdarzenia na magistrali.
    pub fn name(&self) -> &'static str {
        match self {
            Self::DownloadProgress { .. } => EVENT_DOWNLOAD_PROGRESS,
            Self::DownloadFinished { .. } => EVENT_DOWNLOAD_FINISHED,
            Self::DownloadFailed { .. } => EVENT_DOWNLOAD_FAILED,
            Self::Loaded { .. } => EVENT_LOADED,
            Self::Unloaded { .. } => EVENT_UNLOADED,
            Self::BackendFallback { .. } => EVENT_BACKEND_FALLBACK,
            Self::SidecarCrashed { .. } => EVENT_SIDECAR_CRASHED,
        }
    }
}
