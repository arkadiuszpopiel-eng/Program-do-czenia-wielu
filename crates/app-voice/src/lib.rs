//! Tryb głosowy w aplikacji (kategoria `app-*`, wydzielona z `app-core` — limit rozmiaru crate'a):
//! - [`PipelineVoice`] — `VoicePort` z rozmową głosową: mikrofon wł./wył., PTT, wyciszenie, stop
//!   mowy, pigułka (`VoicePill`: kto mówi, poziom, transkrypt częściowy), stan trybu głosowego;
//! - [`ChatReply`] — `ReplySource` na czacie sesji (ta sama sesja, historia append-only,
//!   usłyszany prefiks po przerwaniu);
//! - [`SystemVoice`] — produkcyjna fabryka potoku z `voice-*-impl` (brak modeli/sidecarów →
//!   „głos niedostępny"), [`VoiceEngineFactory`] — podmiana w testach (atrapy, zegar wirtualny);
//! - [`TapBus`] — zdarzenia `voice.pipeline.*` do pętli trybu głosowego i (od `Info`) na magistralę.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

mod engine;
mod pill;
mod port;
mod reply;
mod system;
mod tap;

pub use engine::{IntervalPacer, Pacer, VoiceEngine, VoiceEngineFactory};
pub use pill::{level, mic_state, pill};
pub use port::PipelineVoice;
pub use reply::ChatReply;
pub use system::{MISSING_STT_MODEL, MISSING_STT_SIDECAR, MISSING_TTS, SystemVoice};
pub use tap::TapBus;
