//! Kontrakt `tools-media` (docs/modules/tools-media/SPEC.md, PLAN §7.2 „Multimedia”).
//!
//! - `media_info` — czas, kodeki, wymiary z nagłówków pliku (`lib-media`, bez ffmpeg);
//! - `media_convert` — konwersja przez opcjonalny sidecar `ffmpeg` ([`Transcoder`]: tylko formaty
//!   z listy, wymuszony demuxer wejścia, bez sieci i bez dowolnych argumentów); wynik to **nowy plik**
//!   zapisany przez `undo-journal` (nigdy nadpisanie);
//! - `media_play` — odtwarzanie dźwięku przez `voice-audio` ([`AudioPlayer`]: kolejka mówienia
//!   `speaker` razem z mową agentek, ducking i zatrzymanie przy wywłaszczeniu, kill-switch).

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

mod args;

pub use args::{
    ConvertArgs, ConvertOutput, InfoArgs, InfoOutput, PlayArgs, PlayOutput, TargetFormat,
    check_args, convert_manifest, info_manifest, input_demuxer, manifests, output_path,
    play_manifest, sample_args,
};

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
pub use tokio_util::sync::CancellationToken;

/// Zdarzenie: odczyt informacji o pliku (format, rodzaj — bez ścieżki w treści modelu).
pub const EVENT_INFO: &str = "tool.media.info";
/// Zdarzenie: konwersja (format docelowy, rozmiar, krok cofania).
pub const EVENT_CONVERT: &str = "tool.media.convert";
/// Zdarzenie: odtwarzanie w kolejce (czas, czy czekało).
pub const EVENT_PLAY: &str = "tool.media.play";

/// Zadanie konwersji.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConvertJob {
    /// Plik wejściowy (ścieżka sprawdzona, zgoda `fs.read`).
    pub input: PathBuf,
    /// Format wejścia rozpoznany z nagłówka (`lib-media`).
    pub input_format: String,
    /// Format docelowy.
    pub target: TargetFormat,
    /// Początek fragmentu (ms).
    pub start_ms: Option<u64>,
    /// Długość fragmentu (ms).
    pub duration_ms: Option<u64>,
    /// Dłuższy bok obrazu/wideo (tylko pomniejszanie).
    pub max_side: Option<u32>,
    /// Największy wynik (B).
    pub max_output_bytes: u64,
}

/// Błąd konwersji.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ConvertError {
    /// Sidecar `ffmpeg` nie jest zainstalowany.
    #[error("ffmpeg nie jest zainstalowany ({0})")]
    NotInstalled(String),
    /// Format wejścia lub kombinacja nieobsługiwana.
    #[error("nieobsługiwane: {0}")]
    Unsupported(String),
    /// Wynik ponad limit.
    #[error("wynik ma ponad {0} B")]
    TooLarge(u64),
    /// Przekroczony limit czasu.
    #[error("przekroczony limit czasu konwersji")]
    Timeout,
    /// Anulowano.
    #[error("anulowano")]
    Cancelled,
    /// Błąd procesu albo pliku.
    #[error("konwersja: {0}")]
    Failed(String),
}

/// Konwerter (sidecar `ffmpeg` w Job Object; atrapa w testach). Wywołania blokujące.
pub trait Transcoder: Send + Sync {
    /// Czy konwerter jest gotowy (zainstalowany sidecar).
    fn available(&self) -> Result<(), ConvertError>;
    /// Konwertuje i zwraca bajty wyniku; `cancel` ustawione z zewnątrz przerywa proces.
    fn convert(&self, job: &ConvertJob, cancel: Arc<AtomicBool>) -> Result<Vec<u8>, ConvertError>;
}

/// Klip dźwięku do odtworzenia (PCM `f32` przeplatany).
#[derive(Debug, Clone, PartialEq)]
pub struct AudioClip {
    /// Próbki przeplatane.
    pub samples: Vec<f32>,
    /// Częstotliwość próbkowania (Hz).
    pub sample_rate: u32,
    /// Kanały (1–2).
    pub channels: u16,
    /// Agentka, w której imieniu gra klip (posiadaczka zasobu `speaker`).
    pub agent: String,
    /// Opis dla zdarzeń (nazwa pliku).
    pub label: String,
}

impl AudioClip {
    /// Czas trwania (ms).
    pub fn duration_ms(&self) -> u64 {
        let frames = self.samples.len() as u64 / u64::from(self.channels.max(1));
        frames * 1000 / u64::from(self.sample_rate.max(1))
    }
}

/// Potwierdzenie przyjęcia klipu do kolejki.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlayTicket {
    /// Identyfikator odtwarzania.
    pub id: u64,
    /// Czas klipu (ms).
    pub duration_ms: u64,
    /// Głośnik był zajęty — klip czeka w kolejce mówienia.
    pub queued: bool,
}

/// Błąd odtwarzania.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PlayError {
    /// Wyjście audio niedostępne.
    #[error("głośnik niedostępny: {0}")]
    Unavailable(String),
    /// Klip niepoprawny.
    #[error("klip: {0}")]
    Format(String),
}

/// Odtwarzacz (aplikacja: `voice-audio` + kolejka mówienia `scheduler-lite`).
#[async_trait]
pub trait AudioPlayer: Send + Sync {
    /// Przyjmuje klip: czeka w kolejce mówienia (po mowie agentek), gra w tle; `cancel`
    /// (zatrzymanie przebiegu, kill-switch) przerywa w ≤ 20 ms.
    async fn play(
        &self,
        clip: AudioClip,
        cancel: CancellationToken,
    ) -> Result<PlayTicket, PlayError>;
    /// Zatrzymuje wszystkie klipy (kill-switch); zwraca liczbę zatrzymanych.
    fn stop_all(&self) -> usize;
}

/// Limity (`[tools.media]`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MediaToolsConfig {
    /// Najdłuższy klip do odtworzenia (ms).
    pub max_play_ms: u64,
    /// Największy plik WAV do odtworzenia bez konwersji (B).
    pub max_play_bytes: u64,
    /// Największy wynik konwersji (B).
    pub max_output_bytes: u64,
}

impl Default for MediaToolsConfig {
    fn default() -> Self {
        Self {
            max_play_ms: 10 * 60 * 1000,
            max_play_bytes: 128 * 1024 * 1024,
            max_output_bytes: 512 * 1024 * 1024,
        }
    }
}

/// Testy kontraktowe zestawu `tools-media` (feature `contract-tests`).
#[cfg(feature = "contract-tests")]
pub mod contract_tests {
    use std::sync::Arc;

    use tools_common_contract::{Tool, contract_tests as common};

    use super::{manifests, sample_args};

    /// Manifesty, złe argumenty, anulowanie bez skutków.
    pub async fn run_all(tools: &[Arc<dyn Tool>]) {
        assert_eq!(tools.len(), manifests().len());
        for (tool, manifest) in tools.iter().zip(manifests()) {
            assert_eq!(tool.manifest(), &manifest);
            let name = manifest.name.clone();
            common::run_all(tool.as_ref(), "/", sample_args(&name)).await;
            let bad = serde_json::json!({"path": ""});
            assert!(!tool.call(bad, &common::ctx("/")).await.is_ok(), "{name}");
        }
    }
}

#[cfg(test)]
mod tests;
