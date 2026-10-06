//! Adaptery portów `app-api` na modułach `-impl` (kategoria `app-*`, wydzielone z `app-core`):
//! - [`transfer::TransferAdapter`] — paczki `.alfa` (`transfer-impl`) z natywnymi dialogami
//!   powłoki, podglądem, trybami importu i snapshotem/rollbackiem (sekretów nigdy — CX-a);
//! - [`broker::InprocBroker`] — Broker w procesie (tryb deweloperski, `safety-broker-impl`)
//!   z oknem zatwierdzeń (`ApprovalWindow`) i dziennikiem cofania (`undo-journal-impl`);
//! - [`voice::VoiceAdapter`] — audio (`voice-audio`) i czytanie na głos (`voice-tts`);
//! - [`tts::engines`] — silniki TTS z sidecarów (Pocket TTS, Piper) wykrywane przy każdym użyciu;
//! - [`stt::whisper`] — sidecar STT `whisper-server` (CPU, CUDA z pierwszeństwem, gdy zainstalowany);
//! - [`catalog::ProviderCatalog`] — wbudowany katalog dostawców (`providers-catalog/*.toml`);
//! - [`probe::ProviderProbe`] — test połączenia i wykrywanie modeli kont (`providers-api-impl`);
//! - [`route::RouterBrain`] — „mózg" na trzech rdzeniach Routera (`router-impl`), konta hubu i
//!   model lokalny (`providers-local-impl`, rezydencja `model-residency-impl`);
//! - [`secrets`] — sejf kluczy baz (`KeyVault`) i źródło kluczy API na magazynie `accounts-hub`;
//! - [`late`] — późne wiązanie bazy sesji i indeksera (`sessions` ↔ `search`), [`embedder`] —
//!   lokalny osadzacz leksykalny dla `search`;
//! - [`workdir`] — sprawdzenie katalogu roboczego agentek i deny-listy narzędzi z katalogami
//!   danych Alfy (po rozwiązaniu dowiązań).

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

pub mod broker;
pub mod catalog;
pub mod embedder;
pub mod handles;
pub mod late;
pub mod probe;
pub mod route;
pub mod secrets;
pub mod stt;
pub mod transfer;
pub mod tts;
pub mod voice;
pub mod workdir;

pub use voice::NO_TTS;
