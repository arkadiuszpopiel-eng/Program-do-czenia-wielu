//! Adaptery portów `app-api` na modułach `-impl` (kategoria `app-*`, wydzielone z `app-core`):
//! - [`transfer::TransferAdapter`] — paczki `.alfa` (`transfer-impl`) z natywnymi dialogami
//!   powłoki, podglądem, trybami importu, snapshotem/rollbackiem i jawnym eksportem sekretów;
//! - [`broker::InprocBroker`] — Broker w procesie (tryb deweloperski, `safety-broker-impl`)
//!   z oknem zatwierdzeń (`ApprovalWindow`) i dziennikiem cofania (`undo-journal-impl`);
//! - [`voice::VoiceAdapter`] — audio (`voice-audio`) i czytanie na głos (`voice-tts`);
//! - [`tts::engines`] — silniki TTS z zainstalowanych sidecarów (Pocket TTS, Piper);
//! - [`catalog::ProviderCatalog`] — wbudowany katalog dostawców (`providers-catalog/*.toml`);
//! - [`probe::ProviderProbe`] — test połączenia i wykrywanie modeli kont (`providers-api-impl`).

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

pub mod broker;
pub mod catalog;
pub mod probe;
pub mod transfer;
pub mod tts;
pub mod voice;

pub use voice::NO_TTS;
