//! Czytane zdanie w rozmowie głosowej (plan 43, faza 2). Potok zgłasza fragment odpowiedzi tuż
//! przed oddaniem go do wyjścia audio ([`ReplySource::speaking`]); tu numer tury potoku zamienia
//! się na turę czatu, a `SpeechSegment` wychodzi przez miernik mowy razem z pierwszym poziomem
//! fragmentu — wyróżnienie w oknie nie wyprzedza głosu. Liczba zdań nie jest znana z góry
//! (odpowiedź jeszcze się pisze), więc `of` = [`OPEN`]; koniec wypowiedzi — `index == of`.
//! Przerwanie (barge-in, „stop”) czyści okno samo: wyjście milknie, miernik wysyła ciszę.
//!
//! [`ReplySource::speaking`]: voice_pipeline_contract::ReplySource::speaking

use std::collections::HashMap;
use std::sync::{Mutex, MutexGuard};

use app_api::dto::AlfaEvent;
use app_api::ports::VoiceTurnRef;
use app_modules::speech_meter::SpeechMeter;

/// Liczba zdań nieznana (odpowiedź rozmowy głosowej jeszcze się pisze).
pub const OPEN: u32 = u32::MAX;
/// Najwięcej pamiętanych tur — przerwane, które nie dostały końca, nie rosną bez końca.
const MAX_TURNS: usize = 64;

/// Tura czatu i numer następnego zdania.
struct Voiced {
    turn_id: String,
    next: u32,
}

/// Zdarzenia czytanego zdania dla odpowiedzi rozmowy głosowej.
#[derive(Default)]
pub(crate) struct Speaking {
    meter: Option<SpeechMeter>,
    turns: Mutex<HashMap<u64, Voiced>>,
}

impl Speaking {
    /// Bez miernika (testy, głos bez interfejsu) — nic nie wysyła.
    pub(crate) fn new(meter: Option<SpeechMeter>) -> Self {
        Self {
            meter,
            turns: Mutex::default(),
        }
    }

    fn turns(&self) -> MutexGuard<'_, HashMap<u64, Voiced>> {
        self.turns.lock().unwrap_or_else(|p| p.into_inner())
    }

    /// Tura potoku `turn` odpowiada turze czatu `chat`.
    pub(crate) fn bind(&self, turn: u64, chat: &VoiceTurnRef) {
        if self.meter.is_none() {
            return;
        }
        let mut turns = self.turns();
        if turns.len() >= MAX_TURNS {
            // Najstarsza tura (najniższy numer potoku) — mówiąca teraz jest najnowsza.
            if let Some(oldest) = turns.keys().min().copied() {
                turns.remove(&oldest);
            }
        }
        turns.insert(
            turn,
            Voiced {
                turn_id: app_api::ids::turn_dto(&chat.session, chat.turn),
                next: 0,
            },
        );
    }

    /// Tura przerwana — kolejne zgłoszenia jej nie dotyczą.
    pub(crate) fn forget(&self, turn: u64) {
        self.turns().remove(&turn);
    }

    /// Zgłoszenie potoku: zdanie zaraz zabrzmi (`Some`) albo wypowiedź wybrzmiała (`None`).
    pub(crate) fn speaking(&self, turn: u64, text: Option<&str>) {
        let Some(meter) = &self.meter else {
            return;
        };
        let mut turns = self.turns();
        match text {
            Some(text) => {
                let Some(v) = turns.get_mut(&turn) else {
                    return;
                };
                let index = v.next;
                v.next = v.next.saturating_add(1).min(OPEN - 1);
                meter.mark_next(AlfaEvent::SpeechSegment {
                    turn_id: v.turn_id.clone(),
                    index,
                    of: OPEN,
                    text: text.to_owned(),
                });
            }
            None => {
                if let Some(v) = turns.remove(&turn) {
                    // Znacznik niezagranego (np. pustego) fragmentu nie przejdzie na inną wypowiedź.
                    meter.clear_mark();
                    meter.send(AlfaEvent::SpeechSegment {
                        turn_id: v.turn_id,
                        index: OPEN,
                        of: OPEN,
                        text: String::new(),
                    });
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use std::sync::Arc;
    use std::time::Duration;

    use sessions_contract::{SessionId, TurnId};
    use voice_audio_contract::{AudioFormat, Frame, MediaTime};

    use super::*;

    type Seen = Arc<Mutex<Vec<(String, u32, u32, String)>>>;

    fn meter() -> (SpeechMeter, Seen) {
        let seen: Seen = Arc::default();
        let store = seen.clone();
        let meter = SpeechMeter::with_sink(Arc::new(move |event| {
            if let AlfaEvent::SpeechSegment {
                turn_id,
                index,
                of,
                text,
            } = event
            {
                store.lock().unwrap().push((turn_id, index, of, text));
            }
        }));
        (meter, seen)
    }

    fn chat(turn: u64) -> VoiceTurnRef {
        VoiceTurnRef {
            session: SessionId::from("s1"),
            turn: TurnId(turn),
        }
    }

    fn voice() -> Frame {
        let pcm: Vec<f32> = (0..4_080).map(|i| 0.4 * (i as f32 * 0.05).sin()).collect();
        Frame::new(pcm, AudioFormat::mono(24_000), MediaTime(0)).unwrap()
    }

    #[tokio::test(start_paused = true)]
    async fn sentences_follow_playback_and_end_once() {
        let (meter, seen) = meter();
        let speaking = Speaking::new(Some(meter.clone()));
        speaking.bind(7, &chat(3));
        // Zdanie wychodzi dopiero z pierwszym poziomem fragmentu — nie wcześniej.
        speaking.speaking(7, Some("Kot śpi."));
        assert!(seen.lock().unwrap().is_empty());
        meter.schedule("alfa", &voice());
        speaking.speaking(7, Some("Pies biega."));
        meter.schedule("alfa", &voice());
        tokio::time::sleep(Duration::from_millis(600)).await;
        speaking.speaking(7, None);
        // Po końcu tura jest zapomniana: kolejne zgłoszenia nic nie wysyłają.
        speaking.speaking(7, None);
        speaking.speaking(7, Some("Echo."));
        meter.schedule("alfa", &voice());
        tokio::time::sleep(Duration::from_millis(600)).await;
        let id = app_api::ids::turn_dto(&SessionId::from("s1"), TurnId(3));
        assert_eq!(
            *seen.lock().unwrap(),
            vec![
                (id.clone(), 0, OPEN, "Kot śpi.".to_owned()),
                (id.clone(), 1, OPEN, "Pies biega.".to_owned()),
                (id, OPEN, OPEN, String::new()),
            ]
        );
    }

    #[tokio::test(start_paused = true)]
    async fn unknown_forgotten_or_unmetered_turns_send_nothing() {
        let (meter, seen) = meter();
        let speaking = Speaking::new(Some(meter.clone()));
        // Tura bez powiązania z czatem (np. filler, wznowienie) — bez zdarzeń.
        speaking.speaking(1, Some("Hmm."));
        speaking.speaking(1, None);
        // Tura przerwana — zapomniana.
        speaking.bind(2, &chat(5));
        speaking.forget(2);
        speaking.speaking(2, Some("Zdanie."));
        speaking.speaking(2, None);
        meter.schedule("alfa", &voice());
        tokio::time::sleep(Duration::from_millis(400)).await;
        assert!(seen.lock().unwrap().is_empty());
        // Bez miernika nic nie jest pamiętane.
        let silent = Speaking::new(None);
        silent.bind(3, &chat(1));
        assert!(silent.turns().is_empty());
    }

    #[tokio::test(start_paused = true)]
    async fn unplayed_sentence_does_not_move_to_next_utterance() {
        let (meter, seen) = meter();
        let speaking = Speaking::new(Some(meter.clone()));
        speaking.bind(4, &chat(2));
        // Zdanie zgłoszone, ale fragment nie zagrał (pusty / błąd wyjścia), a wypowiedź się skończyła.
        speaking.speaking(4, Some("Niezagrane."));
        speaking.speaking(4, None);
        // Następna wypowiedź (np. filler) nie niesie starego zdania.
        meter.schedule("alfa", &voice());
        tokio::time::sleep(Duration::from_millis(400)).await;
        let id = app_api::ids::turn_dto(&SessionId::from("s1"), TurnId(2));
        assert_eq!(*seen.lock().unwrap(), vec![(id, OPEN, OPEN, String::new())]);
    }

    #[test]
    fn remembered_turns_are_bounded() {
        let (meter, _) = meter();
        let speaking = Speaking::new(Some(meter));
        for turn in 0..(MAX_TURNS as u64 * 2) {
            speaking.bind(turn, &chat(turn));
        }
        let turns = speaking.turns();
        assert!(turns.len() <= MAX_TURNS);
        // Zostają najnowsze tury, w tym ta, która mówi teraz.
        assert!(turns.contains_key(&(MAX_TURNS as u64 * 2 - 1)));
        assert!(!turns.contains_key(&0));
    }
}
