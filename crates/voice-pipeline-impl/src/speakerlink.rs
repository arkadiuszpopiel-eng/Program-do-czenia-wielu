//! Weryfikacja mówcy w potoku (F5, `voice-speaker`): po finale STT tury głosowej audio tej
//! wypowiedzi (16 kHz po DSP) idzie do wątku weryfikacji (model embeddingu liczy się poza krokiem
//! — krok nie blokuje). Odpowiedź startuje od razu z `SpeakerCheck::Pending` (albo wynikiem, jeśli
//! zdążył), a wynik dochodzi przez `ReplySource::speaker_checked`. Do czasu wyniku — i przy
//! błędzie (za krótka wypowiedź, brak profilu) — tura jest **niezweryfikowana**, więc akcje
//! ryzykowne wymagają potwierdzenia nie-głosem (reguła `VoiceUnverifiedRisky`).

use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::mpsc;

use voice_speaker_contract::{SpeakerCheck, SpeakerError, SpeakerVerifier, Verification};

use crate::pipeline::Pipeline;
use voice_pipeline_contract::PipelineError;

/// Najwięcej zapamiętanych wyników bez tury.
const KEEP_DONE: usize = 16;

type Job = (u64, Vec<f32>);
type Done = (u64, Result<Verification, SpeakerError>);

/// Metadane ostatniego finalu głosowego (do `ReplyRequest::voice`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct VoiceMeta {
    pub stt_permille: u16,
    pub key: Option<u64>,
}

/// Połączenie z wątkiem weryfikacji.
pub(crate) struct SpeakerLink {
    tx: mpsc::Sender<Job>,
    rx: mpsc::Receiver<Done>,
    next: u64,
    done: BTreeMap<u64, SpeakerCheck>,
    turns: BTreeMap<u64, u64>,
}

impl SpeakerLink {
    fn new(verifier: Arc<dyn SpeakerVerifier>) -> std::io::Result<Self> {
        let (tx, jobs) = mpsc::channel::<Job>();
        let (results, rx) = mpsc::channel::<Done>();
        std::thread::Builder::new()
            .name("alfa-voice-speaker".into())
            .spawn(move || {
                // Kończy się, gdy potok upuści nadawcę zadań.
                while let Ok((key, audio)) = jobs.recv() {
                    if results.send((key, verifier.verify(&audio))).is_err() {
                        break;
                    }
                }
            })?;
        Ok(Self {
            tx,
            rx,
            next: 0,
            done: BTreeMap::new(),
            turns: BTreeMap::new(),
        })
    }

    fn submit(&mut self, audio: Vec<f32>) -> Option<u64> {
        self.next += 1;
        self.tx.send((self.next, audio)).ok().map(|()| self.next)
    }

    fn check_for_turn(&mut self, key: u64, turn: u64) -> SpeakerCheck {
        if let Some(c) = self.done.remove(&key) {
            return c;
        }
        self.turns.insert(key, turn);
        SpeakerCheck::Pending
    }

    fn poll(&mut self) -> Vec<(u64, SpeakerCheck)> {
        let mut out = Vec::new();
        while let Ok((key, result)) = self.rx.try_recv() {
            let check = result
                .map(|v| SpeakerCheck::from_verification(&v))
                .unwrap_or(SpeakerCheck::NotChecked);
            match self.turns.remove(&key) {
                Some(turn) => out.push((turn, check)),
                None => {
                    self.done.insert(key, check);
                }
            }
        }
        while self.done.len() > KEEP_DONE {
            self.done.pop_first();
        }
        out
    }
}

impl Pipeline {
    /// Włącza weryfikację mówcy tur głosowych (`voice-speaker`; wątek roboczy).
    pub fn set_speaker_verifier(
        &mut self,
        verifier: Arc<dyn SpeakerVerifier>,
    ) -> Result<(), PipelineError> {
        let link = SpeakerLink::new(verifier).map_err(|e| PipelineError::Component {
            component: "speaker".into(),
            reason: e.to_string(),
        })?;
        self.speaker_link = Some(link);
        Ok(())
    }

    /// Final tury głosowej: pewność STT i zlecenie weryfikacji audio wypowiedzi.
    pub(crate) fn note_voice_final(&mut self, audio: Vec<f32>, stt_confidence: f32) {
        let key = self.speaker_link.as_mut().and_then(|l| l.submit(audio));
        self.st.voice_meta = Some(VoiceMeta {
            stt_permille: (stt_confidence.clamp(0.0, 1.0) * 1000.0).round() as u16,
            key,
        });
    }

    /// Pochodzenie tury głosowej do `ReplyRequest::voice`.
    pub(crate) fn take_voice_provenance(
        &mut self,
        turn: u64,
    ) -> Option<voice_pipeline_contract::VoiceProvenance> {
        let meta = self.st.voice_meta.take()?;
        let speaker = match (meta.key, self.speaker_link.as_mut()) {
            (Some(key), Some(link)) => link.check_for_turn(key, turn),
            _ => SpeakerCheck::NotChecked,
        };
        Some(voice_pipeline_contract::VoiceProvenance {
            stt_confidence_permille: meta.stt_permille,
            speaker,
        })
    }

    /// Wyniki weryfikacji → źródło odpowiedzi.
    pub(crate) fn poll_speaker(&mut self) {
        let Some(link) = self.speaker_link.as_mut() else {
            return;
        };
        for (turn, check) in link.poll() {
            self.replies.speaker_checked(turn, check);
        }
    }
}
