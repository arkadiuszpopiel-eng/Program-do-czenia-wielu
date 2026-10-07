//! `ReplySource` na czacie sesji: tura użytkownika z mowy idzie do tej samej sesji co tekst
//! (historia append-only), odpowiedź strumieniuje się do potoku i do UI jednocześnie, a wynik
//! tury (przerwana → usłyszany prefiks) trafia do historii po zamknięciu przez automat.
//!
//! F5: pochodzenie tury dla klasyfikatora ryzyka/Brokera ([`origin_of`]) — wynik weryfikacji
//! właściciela (czekamy na niego najwyżej [`CHECK_WAIT`], potem tura jest niezweryfikowana);
//! niezweryfikowany głos ma pewność STT ograniczoną do [`UNVERIFIED_CAP_PERMILLE`], więc każda
//! zmiana stanu zlecona nim wymaga potwierdzenia nie-głosem (reguły Jądra bez zmian, destrukcja
//! głosem zawsze pyta nie-głosem). W trakcie dyktowania i czytania tury nie idą do modelu
//! (dyktowany i czytany tekst nie może stać się poleceniem); „stop”/„pauza” sterują czytaniem.

use std::collections::HashMap;
use std::pin::Pin;
use std::sync::{Arc, Mutex, MutexGuard};
use std::task::{Context, Poll};
use std::time::Duration;

use app_api::dto::{SpeakerCheckView, SpeakerDecisionView};
use app_api::ports::{VoiceChat, VoiceChunk, VoiceTurnOrigin, VoiceTurnRef};
use futures_core::Stream;
use providers_contract::CancellationToken;
use tokio::sync::{mpsc, oneshot};
use voice_pipeline_contract::{
    ReplyChunk, ReplyOutcome, ReplyRequest, ReplySource, ReplyStream, VoiceProvenance,
};
use voice_readaloud_contract::ReadControl;
use voice_speaker_contract::{Decision, SpeakerCheck};

use crate::features::F5;

/// Najdłuższe czekanie na wynik weryfikacji mówcy przed startem odpowiedzi.
pub const CHECK_WAIT: Duration = Duration::from_millis(1_500);
/// Pewność STT niezweryfikowanego głosu (< progu 800‰ reguły `VoiceLowConfidence`).
pub const UNVERIFIED_CAP_PERMILLE: u16 = 700;

/// Stan tury potoku (numer automatu → tura czatu).
enum Slot {
    /// Tura czatu już istnieje.
    Started(VoiceTurnRef),
    /// Automat zamknął turę, zanim czat ją utworzył (np. natychmiastowy barge-in).
    Finished(Option<(String, bool)>),
}

/// Wynik weryfikacji dla tury (przed albo po zarejestrowaniu oczekiwania).
enum CheckSlot {
    Waiting(oneshot::Sender<SpeakerCheck>),
    Done(SpeakerCheck),
}

/// Odpowiedzi rozmowy głosowej z czatu sesji.
pub struct ChatReply {
    chat: Arc<dyn VoiceChat>,
    f5: Option<Arc<F5>>,
    slots: Arc<Mutex<HashMap<u64, Slot>>>,
    checks: Arc<Mutex<HashMap<u64, CheckSlot>>>,
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

fn heard_of(outcome: &ReplyOutcome) -> Option<(String, bool)> {
    match outcome {
        ReplyOutcome::Completed => None,
        ReplyOutcome::Interrupted { heard, approximate } => Some((heard.clone(), *approximate)),
    }
}

/// Pochodzenie tury: zweryfikowany właściciel (próg ścisły i włączony przełącznik) — pewność STT
/// z wypowiedzi; inaczej głos niezweryfikowany z pewnością ograniczoną (fail-closed).
pub fn origin_of(voice: Option<&VoiceProvenance>, required: bool) -> VoiceTurnOrigin {
    let Some(p) = voice else {
        return VoiceTurnOrigin::default();
    };
    let verified = required && p.speaker.verified();
    let stt = if verified {
        p.stt_confidence_permille
    } else {
        p.stt_confidence_permille.min(UNVERIFIED_CAP_PERMILLE)
    };
    VoiceTurnOrigin {
        stt_confidence_permille: Some(stt),
        speaker_verified: verified,
    }
}

/// Wynik weryfikacji dla widoku (bez embeddingu).
pub fn check_view(check: &SpeakerCheck) -> SpeakerCheckView {
    match check {
        SpeakerCheck::Checked {
            decision,
            score_permille,
        } => SpeakerCheckView {
            decision: match decision {
                Decision::Verified => SpeakerDecisionView::Verified,
                Decision::Likely => SpeakerDecisionView::Likely,
                Decision::Rejected => SpeakerDecisionView::Rejected,
            },
            score_permille: Some(*score_permille),
        },
        _ => SpeakerCheckView {
            decision: SpeakerDecisionView::NotChecked,
            score_permille: None,
        },
    }
}

/// Komenda czytania w tekście tury („stop”, „pauza”, „dalej”) — gdy czytanie trwa.
fn read_command(text: &str) -> Option<ReadControl> {
    let norm: String = personas_contract::fold(text)
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { ' ' })
        .collect();
    let words: Vec<&str> = norm.split_whitespace().collect();
    let has = |set: &[&str]| words.iter().any(|w| set.contains(w));
    if has(&[
        "stop",
        "stoj",
        "przestan",
        "koniec",
        "zatrzymaj",
        "cisza",
        "wystarczy",
    ]) {
        Some(ReadControl::Stop)
    } else if has(&["pauza", "czekaj", "wstrzymaj"]) {
        Some(ReadControl::Pause)
    } else if has(&["dalej", "wznow", "kontynuuj"]) {
        Some(ReadControl::Resume)
    } else {
        None
    }
}

impl ChatReply {
    /// Źródło odpowiedzi na czacie (bez głosu rozszerzonego: tura głosowa niezweryfikowana).
    pub fn new(chat: Arc<dyn VoiceChat>) -> Self {
        Self::with_features(chat, None)
    }

    /// Źródło ze strażnikami dyktowania/czytania i weryfikacją mówcy.
    pub(crate) fn with_features(chat: Arc<dyn VoiceChat>, f5: Option<Arc<F5>>) -> Self {
        Self {
            chat,
            f5,
            slots: Arc::default(),
            checks: Arc::default(),
        }
    }

    /// Wynik weryfikacji tury: od razu z żądania albo z `speaker_checked` (≤ [`CHECK_WAIT`]).
    fn await_check(&self, turn: u64, voice: Option<&VoiceProvenance>) -> CheckWait {
        let Some(v) = voice else {
            return CheckWait::Ready(SpeakerCheck::NotChecked);
        };
        if v.speaker != SpeakerCheck::Pending {
            return CheckWait::Ready(v.speaker);
        }
        let mut checks = lock(&self.checks);
        if let Some(CheckSlot::Done(c)) = checks.remove(&turn) {
            return CheckWait::Ready(c);
        }
        let (tx, rx) = oneshot::channel();
        checks.insert(turn, CheckSlot::Waiting(tx));
        CheckWait::Pending(rx)
    }
}

enum CheckWait {
    Ready(SpeakerCheck),
    Pending(oneshot::Receiver<SpeakerCheck>),
}

impl ReplySource for ChatReply {
    fn start(&self, request: ReplyRequest, cancel: CancellationToken) -> ReplyStream {
        let (tx, rx) = mpsc::unbounded_channel();
        let chat = self.chat.clone();
        let slots = self.slots.clone();
        let checks = self.checks.clone();
        let f5 = self.f5.clone();
        let wait = self.await_check(request.turn, request.voice.as_ref());
        tokio::spawn(async move {
            if let Some(f5) = f5.as_ref().filter(|f| f.voice_busy()) {
                // Dyktowanie/czytanie: tekst tury nie idzie do modelu; komenda steruje czytaniem.
                if let Some(control) = read_command(&request.text) {
                    f5.control_reading(control);
                }
                lock(&checks).remove(&request.turn);
                let _ = tx.send(ReplyChunk::Done);
                return;
            }
            let check = match wait {
                CheckWait::Ready(c) => c,
                CheckWait::Pending(rx) => {
                    let got = tokio::time::timeout(CHECK_WAIT, rx).await;
                    lock(&checks).remove(&request.turn);
                    got.ok()
                        .and_then(Result::ok)
                        .unwrap_or(SpeakerCheck::NotChecked)
                }
            };
            let required = f5
                .as_ref()
                .is_some_and(|f| f.lock().settings.speaker_required);
            let voice = request.voice.map(|mut v| {
                v.speaker = check;
                v
            });
            if let (Some(f5), Some(_)) = (f5.as_ref(), voice.as_ref()) {
                f5.note_check(check_view(&check));
            }
            let origin = origin_of(voice.as_ref(), required);
            let started = chat
                .voice_turn(request.persona.as_str(), &request.text, origin, cancel)
                .await;
            let mut turn = match started {
                Ok(turn) => turn,
                Err(e) => {
                    let _ = tx.send(ReplyChunk::Failed(e.message));
                    return;
                }
            };
            let early = {
                let mut slots = lock(&slots);
                match slots.remove(&request.turn) {
                    Some(Slot::Finished(heard)) => Some(heard),
                    _ => {
                        slots.insert(request.turn, Slot::Started(turn.turn.clone()));
                        None
                    }
                }
            };
            if let Some(heard) = early {
                chat.voice_finish(turn.turn.clone(), heard).await;
            }
            while let Some(chunk) = turn.chunks.recv().await {
                let (chunk, last) = match chunk {
                    VoiceChunk::Text(t) => (ReplyChunk::Text(t), false),
                    VoiceChunk::Done => (ReplyChunk::Done, true),
                    VoiceChunk::Failed(e) => (ReplyChunk::Failed(e), true),
                };
                if tx.send(chunk).is_err() || last {
                    break;
                }
            }
        });
        Box::pin(ChunkStream { rx })
    }

    fn finish(&self, turn: u64, outcome: ReplyOutcome) {
        let heard = heard_of(&outcome);
        let started = {
            let mut slots = lock(&self.slots);
            match slots.remove(&turn) {
                Some(Slot::Started(r)) => Some(r),
                _ => {
                    slots.insert(turn, Slot::Finished(heard.clone()));
                    None
                }
            }
        };
        if let Some(r) = started {
            let chat = self.chat.clone();
            tokio::spawn(async move { chat.voice_finish(r, heard).await });
        }
    }

    fn speaker_checked(&self, turn: u64, check: SpeakerCheck) {
        let mut checks = lock(&self.checks);
        match checks.remove(&turn) {
            Some(CheckSlot::Waiting(tx)) => {
                let _ = tx.send(check);
            }
            _ => {
                // Wynik przed rejestracją oczekiwania; stare wpisy nie rosną bez końca.
                if checks.len() > 64 {
                    checks.clear();
                }
                checks.insert(turn, CheckSlot::Done(check));
            }
        }
        drop(checks);
        if let Some(f5) = &self.f5 {
            f5.note_check(check_view(&check));
        }
    }
}

/// Strumień fragmentów z kanału.
struct ChunkStream {
    rx: mpsc::UnboundedReceiver<ReplyChunk>,
}

impl Stream for ChunkStream {
    type Item = ReplyChunk;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<ReplyChunk>> {
        self.rx.poll_recv(cx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn prov(check: SpeakerCheck) -> VoiceProvenance {
        VoiceProvenance {
            stt_confidence_permille: 930,
            speaker: check,
        }
    }

    #[test]
    fn unverified_voice_is_capped_and_verified_keeps_confidence() {
        let owner = prov(SpeakerCheck::Checked {
            decision: Decision::Verified,
            score_permille: 800,
        });
        let o = origin_of(Some(&owner), true);
        assert!(o.speaker_verified);
        assert_eq!(o.stt_confidence_permille, Some(930));
        // Przełącznik wyłączony — nawet właściciel jest niezweryfikowany.
        let o = origin_of(Some(&owner), false);
        assert!(!o.speaker_verified);
        assert_eq!(o.stt_confidence_permille, Some(UNVERIFIED_CAP_PERMILLE));
        for check in [
            SpeakerCheck::Pending,
            SpeakerCheck::NotChecked,
            SpeakerCheck::Checked {
                decision: Decision::Likely,
                score_permille: 500,
            },
            SpeakerCheck::Checked {
                decision: Decision::Rejected,
                score_permille: 100,
            },
        ] {
            let o = origin_of(Some(&prov(check)), true);
            assert!(!o.speaker_verified, "{check:?}");
            assert_eq!(o.stt_confidence_permille, Some(UNVERIFIED_CAP_PERMILLE));
        }
        assert_eq!(origin_of(None, true), VoiceTurnOrigin::default());
    }

    #[test]
    fn read_commands_are_recognized() {
        assert_eq!(read_command("Stop!"), Some(ReadControl::Stop));
        assert_eq!(read_command("Alfa, przestań"), Some(ReadControl::Stop));
        assert_eq!(read_command("pauza"), Some(ReadControl::Pause));
        assert_eq!(read_command("czytaj dalej"), Some(ReadControl::Resume));
        assert_eq!(read_command("jaka jest pogoda"), None);
    }
}
