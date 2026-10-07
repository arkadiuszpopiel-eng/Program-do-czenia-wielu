//! Tryb głosowy w aplikacji (kategoria `app-*`, wydzielona z `app-core` — limit rozmiaru crate'a):
//! - [`PipelineVoice`] — `VoicePort` z rozmową głosową: mikrofon wł./wył., PTT, wyciszenie, stop
//!   mowy, pigułka (`VoicePill`: kto mówi, poziom, transkrypt częściowy), stan trybu głosowego;
//! - [`ChatReply`] — `ReplySource` na czacie sesji (ta sama sesja, historia append-only,
//!   usłyszany prefiks po przerwaniu; F5: pochodzenie tury z weryfikacją właściciela);
//! - [`SystemVoice`] — produkcyjna fabryka potoku z `voice-*-impl` (brak modeli/sidecarów →
//!   „głos niedostępny"), [`VoiceEngineFactory`] — podmiana w testach (atrapy, zegar wirtualny);
//! - [`TapBus`] — zdarzenia `voice.pipeline.*` do pętli trybu głosowego i (od `Info`) na magistralę;
//! - głos rozszerzony F5 ([`FeatureFactory`], [`FeatureDeps`]): słowa wywoławcze z bramką
//!   właściciela (domyślnie wyłączone), kreator rejestracji głosu i weryfikacja dla akcji
//!   ryzykownych, dyktowanie do dowolnej aplikacji (odmowa w polach haseł), czytanie zaznaczenia,
//!   dokumentu i schowka z kolejką; S2S w chmurze — tylko stan „wymaga klucza”.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

mod engine;
mod features;
mod pill;
mod port;
mod reply;
mod runloop;
mod system;
mod system_f5;
mod tap;

pub use engine::{IntervalPacer, Pacer, VoiceEngine, VoiceEngineFactory, VoiceRuntime};
pub use features::{
    DesktopDeps, FeatureDeps, FeatureFactory, ReadAudio, Settings, WakeCalibration,
};
pub use pill::{level, mic_state, pill};
pub use port::PipelineVoice;
pub use reply::{CHECK_WAIT, ChatReply, UNVERIFIED_CAP_PERMILLE, origin_of};
pub use system::{MISSING_STT_MODEL, MISSING_STT_SIDECAR, MISSING_TTS, SystemVoice};
pub use system_f5::CALIBRATION_FILE;
pub use tap::TapBus;
