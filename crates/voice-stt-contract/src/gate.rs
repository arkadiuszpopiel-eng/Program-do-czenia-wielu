//! Bramka VAD przed wysłaniem do STT i wspólne reguły (prywatność, prompt hotwords).

use voice_audio_contract::Frame;
use voice_vad_contract::EnergyDetector;

use crate::{SttCfg, SttEngine, SttError};
use providers_contract::PrivacyTag;

/// Bufor wypowiedzi (16 kHz mono) z licznikiem mowy (detektor energii, okna 10 ms).
#[derive(Debug, Clone, Default)]
pub struct UtteranceAudio {
    samples: Vec<f32>,
    detector: EnergyDetector,
    speech_ms: u32,
    partial_at: usize,
}

impl UtteranceAudio {
    /// Dopisuje ramkę (sprawdza format).
    pub fn push(&mut self, frame: &Frame) -> Result<(), SttError> {
        if frame.format.sample_rate != 16_000 || frame.format.channels != 1 {
            return Err(SttError::Format(format!(
                "{} Hz × {} (wymagane 16 kHz mono)",
                frame.format.sample_rate, frame.format.channels
            )));
        }
        let start = self.samples.len();
        self.samples.extend_from_slice(&frame.pcm);
        let aligned = start - start % 160;
        // Okno [aligned, start) było wcześniej niepełne — liczymy je teraz (każde okno raz).
        for w in self.samples[aligned..].chunks_exact(160) {
            if self.detector.prob(w) >= 0.5 {
                self.speech_ms += 10;
            }
        }
        Ok(())
    }

    /// Próbki.
    pub fn samples(&self) -> &[f32] {
        &self.samples
    }

    /// Długość (ms).
    pub fn duration_ms(&self) -> u32 {
        (self.samples.len() / 16) as u32
    }

    /// Mowa wg detektora energii (ms).
    pub fn speech_ms(&self) -> u32 {
        self.speech_ms
    }

    /// Czy przybyło dość audio na kolejny partial (`every_ms`); oznacza moment partiala.
    pub fn take_partial_due(&mut self, every_ms: u32) -> bool {
        let every = every_ms as usize * 16;
        if self.samples.len() >= self.partial_at + every && self.samples.len() >= 16_000 {
            self.partial_at = self.samples.len();
            true
        } else {
            false
        }
    }
}

/// Czy silnik może przyjąć audio przy danym tagu prywatności (sprawdzane przed ruchem sieciowym).
pub fn check_privacy(cfg: &SttCfg) -> Result<(), SttError> {
    match (&cfg.engine, cfg.privacy) {
        (SttEngine::Cloud { .. }, PrivacyTag::Private) => Err(SttError::PrivacyBlocked),
        _ => Ok(()),
    }
}

/// Prompt początkowy whisper z hotwords (biasing): „Alfa, Beta, …”.
pub fn hotword_prompt(hotwords: &[String]) -> Option<String> {
    let words: Vec<&str> = hotwords
        .iter()
        .map(|w| w.trim())
        .filter(|w| !w.is_empty())
        .collect();
    (!words.is_empty()).then(|| format!("{}.", words.join(", ")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use voice_audio_contract::MediaTime;
    use voice_audio_contract::synth::{sine, white_noise};

    #[test]
    fn counts_speech_and_partials() {
        let mut u = UtteranceAudio::default();
        u.push(&Frame::mono(
            white_noise(1, 8_000, 0.001),
            16_000,
            MediaTime::ZERO,
        ))
        .unwrap();
        let tone: Vec<f32> = sine(220.0, 16_000, 1.0, 0.3);
        for c in tone.chunks(333) {
            u.push(&Frame::mono(c.to_vec(), 16_000, MediaTime::ZERO))
                .unwrap();
        }
        assert_eq!(u.duration_ms(), 1_500);
        assert!(
            u.speech_ms() >= 950 && u.speech_ms() <= 1_010,
            "{}",
            u.speech_ms()
        );
        assert!(u.take_partial_due(1_000));
        assert!(!u.take_partial_due(1_000));
        assert_eq!(u.samples().len(), 24_000);
        assert!(
            u.push(&Frame::mono(vec![0.0; 480], 48_000, MediaTime::ZERO))
                .is_err()
        );
    }

    #[test]
    fn privacy_and_prompt() {
        let mut cfg = SttCfg {
            privacy: PrivacyTag::Private,
            ..SttCfg::default()
        };
        assert!(
            check_privacy(&cfg).is_ok(),
            "lokalny silnik w sesji prywatnej"
        );
        cfg.engine = SttEngine::Cloud {
            provider: crate::CloudStt::Soniox,
            account: "a".into(),
            model: "m".into(),
        };
        assert_eq!(check_privacy(&cfg), Err(SttError::PrivacyBlocked));
        assert_eq!(
            hotword_prompt(&["Alfa".into(), " ".into(), "Delta".into()]).as_deref(),
            Some("Alfa, Delta.")
        );
        assert_eq!(hotword_prompt(&[]), None);
    }
}
