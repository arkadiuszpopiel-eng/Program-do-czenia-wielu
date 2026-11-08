//! Atrapa `voice-s2s` (docs/modules/voice-s2s/SPEC.md): skryptowany klient speech-to-speech
//! bez sieci. Odpowiedź na zatwierdzoną turę jest strumieniowana po jednej porcji audio
//! (100 ms) na `poll`; `truncate` obcina tekst odpowiedzi w historii dostawcy proporcjonalnie
//! do usłyszanego audio (bez notki — `NativeTruncate`).

use std::collections::VecDeque;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use async_trait::async_trait;
use personas_contract::PersonaId;
use providers_contract::InterruptionRendering;
use voice_audio_contract::{Frame, MediaTime};
use voice_s2s_contract::contract_tests::S2sDriver;
use voice_s2s_contract::{
    ItemId, S2sBusEvent, S2sCfg, S2sClient, S2sError, S2sEvent, S2sProvider, S2sRole, S2sSession,
};

/// Długość porcji audio odpowiedzi (ms).
pub const DELTA_MS: u32 = 100;
/// Próg RMS „mowy” dla VAD serwera atrapy.
const SPEECH_RMS: f32 = 0.05;

/// Skryptowana tura: co „usłyszał” dostawca i co odpowiada.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScriptedReply {
    /// Transkrypcja wejścia użytkownika.
    pub user_text: String,
    /// Tekst odpowiedzi asystentki.
    pub reply_text: String,
    /// Długość audio odpowiedzi (ms; co najmniej dwie porcje).
    pub reply_ms: u32,
}

#[derive(Debug, Default)]
struct Shared {
    replies: VecDeque<ScriptedReply>,
    connections: u32,
    audio_sent_ms: u64,
    instructions: Option<String>,
    history: Vec<(S2sRole, String)>,
    fail_connect: Option<S2sError>,
}

/// Skryptowany klient S2S (klonowalny uchwyt do wspólnego stanu).
#[derive(Debug, Clone, Default)]
pub struct FakeS2sClient {
    shared: Arc<Mutex<Shared>>,
}

impl FakeS2sClient {
    /// Nowy klient.
    pub fn new() -> Self {
        Self::default()
    }

    fn lock(&self) -> MutexGuard<'_, Shared> {
        self.shared.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Dopisuje odpowiedź na kolejną turę (bez skryptu: „Rozumiem.”, 400 ms).
    pub fn script_reply(&self, user_text: &str, reply_text: &str, reply_ms: u32) {
        self.lock().replies.push_back(ScriptedReply {
            user_text: user_text.into(),
            reply_text: reply_text.into(),
            reply_ms,
        });
    }

    /// Następne `connect` kończy się błędem dostawcy.
    pub fn fail_next_connect(&self, err: S2sError) {
        self.lock().fail_connect = Some(err);
    }

    /// Liczba udanych połączeń z „chmurą”.
    pub fn connections(&self) -> u32 {
        self.lock().connections
    }

    /// Łącznie wysłanego audio (ms, wszystkie sesje).
    pub fn audio_sent_ms(&self) -> u64 {
        self.lock().audio_sent_ms
    }

    /// Instrukcja systemowa ostatniej sesji.
    pub fn last_instructions(&self) -> Option<String> {
        self.lock().instructions.clone()
    }

    /// Historia rozmowy po stronie dostawcy (ostatnia sesja, po obcięciach).
    pub fn history(&self) -> Vec<(S2sRole, String)> {
        self.lock().history.clone()
    }
}

impl S2sDriver for FakeS2sClient {
    /// Atrapa strumieniuje jedną porcję na `poll` — upływ czasu nie gra roli.
    fn advance(&self, _ms: u64) {}
}

#[async_trait]
impl S2sClient for FakeS2sClient {
    async fn connect(
        &self,
        cfg: &S2sCfg,
        persona: &PersonaId,
        instructions: &str,
    ) -> Result<Box<dyn S2sSession>, S2sError> {
        cfg.validate(persona)?;
        let mut sh = self.lock();
        if let Some(e) = sh.fail_connect.take() {
            return Err(e);
        }
        sh.connections += 1;
        sh.instructions = Some(instructions.to_owned());
        sh.history.clear();
        drop(sh);
        Ok(Box::new(FakeSession {
            client: self.clone(),
            provider: cfg.provider,
            sample_rate: cfg.sample_rate,
            buffer_ms: 0,
            sent_ms: 0,
            pending: VecDeque::new(),
            current: None,
            items: Vec::new(),
            bus: vec![S2sBusEvent::SessionStarted {
                provider: cfg.provider,
                model: cfg.model.clone(),
                persona: persona.clone(),
            }],
            barge_flagged: false,
            closed: false,
        }))
    }
}

#[derive(Debug)]
struct ItemRec {
    id: ItemId,
    history_index: usize,
    total_ms: u32,
    delivered_ms: u32,
}

struct FakeSession {
    client: FakeS2sClient,
    provider: S2sProvider,
    sample_rate: u32,
    buffer_ms: u64,
    sent_ms: u64,
    pending: VecDeque<S2sEvent>,
    current: Option<ItemId>,
    items: Vec<ItemRec>,
    bus: Vec<S2sBusEvent>,
    barge_flagged: bool,
    closed: bool,
}

fn rms(pcm: &[f32]) -> f32 {
    if pcm.is_empty() {
        return 0.0;
    }
    (pcm.iter().map(|v| v * v).sum::<f32>() / pcm.len() as f32).sqrt()
}

fn tone(sample_rate: u32, ms: u32, at_ms: u64) -> Frame {
    let n = (u64::from(sample_rate) * u64::from(ms) / 1000) as usize;
    let pcm: Vec<f32> = (0..n)
        .map(|i| 0.2 * (2.0 * std::f32::consts::PI * 180.0 * i as f32 / sample_rate as f32).sin())
        .collect();
    Frame::mono(
        pcm,
        sample_rate,
        MediaTime::from_samples(at_ms * u64::from(sample_rate) / 1000, sample_rate),
    )
}

/// Prefiks tekstu na granicy słowa proporcjonalny do `heard / total`.
fn heard_prefix(text: &str, heard_ms: u32, total_ms: u32) -> String {
    let words: Vec<&str> = text.split_whitespace().collect();
    let keep = (words.len() as u64 * u64::from(heard_ms) / u64::from(total_ms.max(1))) as usize;
    words[..keep.min(words.len())].join(" ")
}

impl FakeSession {
    fn ensure_open(&self) -> Result<(), S2sError> {
        if self.closed {
            Err(S2sError::Closed)
        } else {
            Ok(())
        }
    }

    fn drop_current(&mut self) -> Option<ItemId> {
        let item = self.current.take()?;
        self.pending.retain(|e| match e {
            S2sEvent::AudioDelta { item: i, .. } | S2sEvent::ResponseDone { item: i } => *i != item,
            S2sEvent::Transcript {
                role: S2sRole::Assistant,
                ..
            } => false,
            _ => true,
        });
        self.barge_flagged = false;
        Some(item)
    }
}

#[async_trait]
impl S2sSession for FakeSession {
    async fn send_audio(&mut self, frame: &Frame) -> Result<(), S2sError> {
        self.ensure_open()?;
        if frame.format.channels != 1 || frame.format.sample_rate != self.sample_rate {
            return Err(S2sError::InvalidConfig(format!(
                "oczekiwane mono {} Hz",
                self.sample_rate
            )));
        }
        let ms = frame.frames() as u64 * 1000 / u64::from(self.sample_rate);
        self.buffer_ms += ms;
        self.sent_ms += ms;
        self.client.lock().audio_sent_ms += ms;
        self.bus.push(S2sBusEvent::AudioSent {
            provider: self.provider,
            audio_ms: u32::try_from(ms).unwrap_or(u32::MAX),
        });
        if self.current.is_some() && !self.barge_flagged && rms(&frame.pcm) > SPEECH_RMS {
            self.barge_flagged = true;
            self.pending.push_front(S2sEvent::UserSpeechStarted);
        }
        Ok(())
    }

    async fn commit_turn(&mut self) -> Result<(), S2sError> {
        self.ensure_open()?;
        if self.buffer_ms == 0 {
            return Err(S2sError::Provider("pusty bufor audio".into()));
        }
        self.buffer_ms = 0;
        self.drop_current();
        let reply = self
            .client
            .lock()
            .replies
            .pop_front()
            .unwrap_or(ScriptedReply {
                user_text: "(mowa)".into(),
                reply_text: "Rozumiem.".into(),
                reply_ms: 400,
            });
        let item = ItemId(format!("item-{}", self.items.len() + 1));
        let deltas = reply.reply_ms.div_ceil(DELTA_MS).max(2);
        let history_index = {
            let mut sh = self.client.lock();
            sh.history.push((S2sRole::User, reply.user_text.clone()));
            sh.history
                .push((S2sRole::Assistant, reply.reply_text.clone()));
            sh.history.len() - 1
        };
        self.items.push(ItemRec {
            id: item.clone(),
            history_index,
            total_ms: deltas * DELTA_MS,
            delivered_ms: 0,
        });
        self.pending.push_back(S2sEvent::Transcript {
            role: S2sRole::User,
            text: reply.user_text,
            is_final: true,
        });
        for k in 0..deltas {
            self.pending.push_back(S2sEvent::AudioDelta {
                item: item.clone(),
                audio: tone(self.sample_rate, DELTA_MS, u64::from(k * DELTA_MS)),
            });
        }
        self.pending.push_back(S2sEvent::Transcript {
            role: S2sRole::Assistant,
            text: reply.reply_text,
            is_final: true,
        });
        self.pending
            .push_back(S2sEvent::ResponseDone { item: item.clone() });
        self.current = Some(item);
        Ok(())
    }

    async fn cancel_response(&mut self) -> Result<(), S2sError> {
        self.ensure_open()?;
        if let Some(item) = self.drop_current() {
            self.pending.push_back(S2sEvent::ResponseDone { item });
        }
        Ok(())
    }

    async fn truncate(&mut self, item: &ItemId, audio_end_ms: u32) -> Result<(), S2sError> {
        self.ensure_open()?;
        if self.provider.interruption() != InterruptionRendering::NativeTruncate {
            return Err(S2sError::Unsupported);
        }
        let Some(rec) = self
            .items
            .iter()
            .find(|r| r.id == *item && r.delivered_ms > 0)
        else {
            return Err(S2sError::UnknownItem(item.0.clone()));
        };
        let end = audio_end_ms.min(rec.delivered_ms);
        let mut sh = self.client.lock();
        if let Some((_, text)) = sh.history.get_mut(rec.history_index) {
            *text = heard_prefix(text, end, rec.total_ms);
        }
        drop(sh);
        self.bus.push(S2sBusEvent::Truncated {
            item: item.clone(),
            audio_end_ms: end,
        });
        Ok(())
    }

    fn poll(&mut self) -> Vec<S2sEvent> {
        let mut out = Vec::new();
        while let Some(e) = self.pending.pop_front() {
            let is_audio = match &e {
                S2sEvent::AudioDelta { item, audio } => {
                    let ms =
                        u32::try_from(audio.frames() as u64 * 1000 / u64::from(self.sample_rate))
                            .unwrap_or(u32::MAX);
                    if let Some(r) = self.items.iter_mut().find(|r| r.id == *item) {
                        r.delivered_ms = r.delivered_ms.saturating_add(ms);
                    }
                    true
                }
                S2sEvent::ResponseDone { item } => {
                    if self.current.as_ref() == Some(item) {
                        self.current = None;
                        self.barge_flagged = false;
                    }
                    false
                }
                _ => false,
            };
            out.push(e);
            if is_audio {
                break;
            }
        }
        out
    }

    fn take_bus_events(&mut self) -> Vec<S2sBusEvent> {
        std::mem::take(&mut self.bus)
    }

    async fn close(&mut self) {
        if self.closed {
            return;
        }
        self.closed = true;
        self.pending.clear();
        self.current = None;
        self.bus.push(S2sBusEvent::SessionClosed {
            audio_sent_ms: self.sent_ms,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::heard_prefix;

    #[test]
    fn prefix_is_word_aligned() {
        assert_eq!(heard_prefix("Ala ma kota i psa", 400, 1_000), "Ala ma");
        assert_eq!(heard_prefix("Ala ma kota", 0, 1_000), "");
        assert_eq!(heard_prefix("Ala ma kota", 5_000, 1_000), "Ala ma kota");
    }
}
