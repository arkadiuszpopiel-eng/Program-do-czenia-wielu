//! `VoicePort` na `voice-audio` (WASAPI na Windows; lista urządzeń, test mikrofonu z poziomem
//! `MicLevel` ≤ 30/s) i `voice-tts` (czytanie na głos głosem agentki, głosy v0 bez kluczy).
//! Rozmowa głosowa (mikrofon wł., wyciszenie, pigułka) należy do potoku `voice-pipeline`.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use async_trait::async_trait;
use personas_contract::PersonaId;
use providers_contract::PrivacyTag;
use voice_audio_contract::{AudioIo, DeviceId, DeviceKind, SourceId, StreamConfig};
use voice_tts_contract::{CancelToken, SpeechStyle, Tts, TtsRequest};

use app_api::dto::{AlfaEvent, AudioDevice, LocalizedText, ToastKind};
use app_api::error::{AppError, ErrorCode};
use app_api::events::EventHub;
use app_api::ports::VoicePort;

/// Brak sidecara TTS (Pocket TTS / Piper).
pub const NO_TTS: &str = "Brak silnika TTS — pobierz go w Ustawieniach → Głos.";

/// Okres próbkowania poziomu mikrofonu (≤ 30 zdarzeń/s, PLAN §14.7).
const LEVEL_PERIOD: Duration = Duration::from_millis(34);

#[derive(Default)]
struct State {
    mic_test: Option<Arc<AtomicBool>>,
    speech: Option<(u64, CancelToken)>,
}

/// Głos: audio + TTS.
pub struct VoiceAdapter {
    io: Arc<dyn AudioIo>,
    tts: Option<Arc<dyn Tts>>,
    events: EventHub,
    next: AtomicU64,
    state: Arc<Mutex<State>>,
}

fn audio_error(what: &str, e: impl std::fmt::Display) -> AppError {
    AppError::new(ErrorCode::Unavailable, format!("{what}: {e}"))
}

fn lock(state: &Mutex<State>) -> MutexGuard<'_, State> {
    state.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Poziom 0..1 z RMS ramki (skala logarytmiczna −60…0 dBFS).
fn level(pcm: &[f32]) -> f64 {
    if pcm.is_empty() {
        return 0.0;
    }
    let energy: f64 = pcm
        .iter()
        .map(|s| f64::from(*s) * f64::from(*s))
        .sum::<f64>();
    let rms = (energy / pcm.len() as f64).sqrt();
    if rms <= 1e-6 {
        return 0.0;
    }
    ((20.0 * rms.log10() + 60.0) / 60.0).clamp(0.0, 1.0)
}

impl VoiceAdapter {
    /// Głos na `AudioIo` i opcjonalnym TTS (`None` — brak sidecara); zdarzenia do UI.
    pub fn new(io: Arc<dyn AudioIo>, tts: Option<Arc<dyn Tts>>, events: EventHub) -> Self {
        Self {
            io,
            tts,
            events,
            next: AtomicU64::new(1),
            state: Arc::default(),
        }
    }

    fn toast(events: &EventHub, pl: String) {
        events.emit(AlfaEvent::Toast {
            kind: ToastKind::Error,
            message: LocalizedText::new(pl, "Reading aloud failed."),
        });
    }
}

#[async_trait]
impl VoicePort for VoiceAdapter {
    async fn devices(&self) -> Result<Option<Vec<AudioDevice>>, AppError> {
        // Bez audio systemu (poza Windows) — lista z `device-profile`.
        let Ok(devices) = self.io.devices() else {
            return Ok(None);
        };
        Ok(Some(
            devices
                .into_iter()
                .filter(|d| d.kind == DeviceKind::Input)
                .map(|d| AudioDevice {
                    id: d.id.0,
                    name: d.name,
                    default: d.is_default,
                })
                .collect(),
        ))
    }

    async fn start_mic_test(&self, device: Option<String>) -> Result<(), AppError> {
        self.stop_mic_test().await?;
        let id = device.map(DeviceId);
        let mut input = self
            .io
            .open_input(id.as_ref(), &StreamConfig::input_default())
            .map_err(|e| audio_error("Test mikrofonu", e))?;
        let stop = Arc::new(AtomicBool::new(false));
        lock(&self.state).mic_test = Some(stop.clone());
        let events = self.events.clone();
        tokio::spawn(async move {
            let mut tick = tokio::time::interval(LEVEL_PERIOD);
            while !stop.load(Ordering::SeqCst) {
                tick.tick().await;
                let mut peak: Option<f64> = None;
                while let Some(frame) = input.read() {
                    let l = level(&frame.pcm);
                    peak = Some(peak.map_or(l, |p| p.max(l)));
                }
                if let Some(level) = peak {
                    events.emit(AlfaEvent::MicLevel { level });
                }
            }
            events.emit(AlfaEvent::MicLevel { level: 0.0 });
        });
        Ok(())
    }

    async fn stop_mic_test(&self) -> Result<(), AppError> {
        if let Some(stop) = lock(&self.state).mic_test.take() {
            stop.store(true, Ordering::SeqCst);
        }
        Ok(())
    }

    async fn set_mic_enabled(&self, enabled: bool) -> Result<(), AppError> {
        if enabled {
            return Err(AppError::unavailable("Rozmowa głosowa", "voice-pipeline"));
        }
        Ok(())
    }

    async fn set_muted(&self, _muted: bool) -> Result<(), AppError> {
        Ok(())
    }

    async fn stop_speech(&self) -> Result<(), AppError> {
        if let Some((utterance, cancel)) = lock(&self.state).speech.take() {
            cancel.cancel();
            if let Some(tts) = &self.tts {
                tts.stop(utterance);
            }
        }
        Ok(())
    }

    async fn read_aloud(&self, agent: &str, text: &str) -> Result<(), AppError> {
        let tts = self
            .tts
            .clone()
            .ok_or_else(|| AppError::new(ErrorCode::Unavailable, NO_TTS))?;
        if text.trim().is_empty() {
            return Err(AppError::invalid(
                "Ta wiadomość nie ma tekstu do przeczytania.",
            ));
        }
        self.stop_speech().await?;
        let utterance = self.next.fetch_add(1, Ordering::SeqCst);
        let persona = PersonaId::new(agent);
        let cancel = CancelToken::new();
        let request = TtsRequest {
            utterance,
            persona: persona.clone(),
            text: text.to_owned(),
            style: SpeechStyle::default(),
            cacheable: false,
            privacy: PrivacyTag::Normal,
        };
        let mut stream = tts
            .synth(request, cancel.clone())
            .await
            .map_err(|e| audio_error("Czytanie na głos", e))?;
        let mut output = self
            .io
            .open_output(None, &StreamConfig::output_default())
            .map_err(|e| audio_error("Czytanie na głos — głośnik", e))?;
        lock(&self.state).speech = Some((utterance, cancel.clone()));
        let events = self.events.clone();
        let state = self.state.clone();
        tokio::spawn(async move {
            let source = SourceId::Tts(persona);
            let mut played = Duration::ZERO;
            while let Some(chunk) = stream.recv().await {
                if cancel.is_cancelled() {
                    break;
                }
                match chunk {
                    Ok(chunk) => {
                        played += chunk.audio.duration();
                        if let Err(e) = output.play(&source, utterance, &chunk.audio) {
                            Self::toast(&events, format!("Czytanie na głos: {e}"));
                            break;
                        }
                        if chunk.is_last {
                            break;
                        }
                    }
                    Err(e) => {
                        Self::toast(&events, format!("Czytanie na głos: {e}"));
                        break;
                    }
                }
            }
            let _ = output.end_utterance(utterance);
            // Wyjście żyje do końca odtwarzania (albo do „Stop").
            let deadline = tokio::time::Instant::now() + played;
            while tokio::time::Instant::now() < deadline && !cancel.is_cancelled() {
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
            if cancel.is_cancelled() {
                let _ = output.stop_all();
            }
            let mut st = lock(&state);
            if st.speech.as_ref().is_some_and(|(u, _)| *u == utterance) {
                st.speech = None;
            }
        });
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn level_maps_rms_to_unit_range() {
        assert!(level(&[]).abs() < f64::EPSILON);
        assert!(level(&[0.0; 480]).abs() < f64::EPSILON);
        assert!((level(&[1.0; 480]) - 1.0).abs() < 1e-9);
        let quiet = level(&[0.01; 480]);
        assert!(quiet > 0.0 && quiet < 0.5, "{quiet}");
    }
}
