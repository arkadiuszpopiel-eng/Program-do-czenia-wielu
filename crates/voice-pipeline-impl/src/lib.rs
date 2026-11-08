//! Implementacja modułu `voice-pipeline` (docs/modules/voice-pipeline/SPEC.md): runtime, który
//! składa kontrakty `voice-*` w rozmowę głosową i wykonuje polecenia automatu dialogu.
//!
//! Wątki i kolejki: wątek RT urządzenia (w `voice-audio`, bez alokacji) ↔ kolejki SPSC
//! (przechwytywanie, mikser, referencja AEC) ↔ **wątek przetwarzania** = [`Pipeline::step`] co
//! ~10 ms (DSP, reguła echa, VAD, koniec tury, komendy, automat) — wolne operacje (STT, LLM, TTS,
//! magistrala) są przyszłościami odpytywanymi w kolejnych krokach, nigdy nie blokują kroku.
//! Punkty anulowania: `CancellationToken` odpowiedzi (barge-in, „stop”, `Esc`), `CancelToken`
//! syntezy per wypowiedź (`StopTts`), `stop_all` wyjścia (cisza ≤ 20 ms), `cancel` wypowiedzi STT.
//!
//! Składniki: [`Pipeline`] (z [`PipelineParts`]), [`SchedSpeakerLock`] (głośnik na
//! `scheduler-lite`), [`EchoGate`] (reguła echa obok AEC), [`ProviderReply`] (odpowiedzi
//! z `ModelProvider` + historia append-only), [`MonotonicClock`], [`eval`] (zestaw F2: manifest,
//! WER PL, recall „stop/anuluj”, precision backchannelu, fałszywe przerwania/h, prefiks).
//! Słowa wywoławcze (F5): [`Pipeline::arm_wake_words`] — przed wykryciem audio mikrofonu trafia
//! wyłącznie do nasłuchu `voice-wake` (bufor ~2 s), nie do VAD/STT ani magistrali.
//! Weryfikacja mówcy (F5): [`Pipeline::set_speaker_verifier`] — wynik w `ReplyRequest::voice`
//! (pewność STT + `SpeakerCheck`) dla klasyfikatora ryzyka.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

mod clock;
mod cmd;
mod commands;
mod echo;
pub mod eval;
mod mic;
mod outbox;
mod pipeline;
mod poll;
mod provider_reply;
mod reply;
mod residency;
mod speaker;
mod speakerlink;
mod speech;
mod step;
mod stt;
mod user;
mod wakeword;

pub use clock::MonotonicClock;
pub use echo::EchoGate;
pub use pipeline::{Pipeline, PipelineParts, PlayRecord, TraceEntry};
pub use provider_reply::{PromptFn, ProviderReply};
pub use reply::raw_prefix;
pub use residency::VOICE_OWNERS;
pub use speaker::SchedSpeakerLock;

/// Treść `module.toml` tego modułu.
pub const MODULE_TOML: &str = include_str!("../module.toml");
