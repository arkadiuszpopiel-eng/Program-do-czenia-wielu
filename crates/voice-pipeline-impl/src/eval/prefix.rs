//! Wierny tryb STT runnera na prawdziwym modelu: [`PrefixStt`] zbiera audio wypowiedzi i na każde
//! `partial_now` uruchamia `whisper-cli` na audio „do teraz” (jak szybki przebieg `voice-stt`
//! w potoku), a final — na całości. Pliki tymczasowe (prefiksy nagrań = dane biometryczne)
//! powstają tylko w katalogu roboczym wskazanym przez użytkownika i są usuwane od razu.

use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use async_trait::async_trait;
use device_profile_contract::Backend;
use voice_audio_contract::{Frame, PIPELINE_RATE};
use voice_stt_contract::{Health, Stt, SttCfg, SttError, SttEvent, Transcript, UtteranceId, Word};

use crate::eval::audio::wav16;
use crate::eval::manifest::Segment;
use crate::eval::timed::whisper_words;

/// Wywołanie `whisper-cli` (whisper.cpp).
#[derive(Debug, Clone)]
pub struct WhisperCli {
    /// Plik wykonywalny.
    pub exe: PathBuf,
    /// Model GGML.
    pub model: PathBuf,
    /// Język (`pl`).
    pub lang: String,
    /// Katalog roboczy na prefiksy (lokalny, poza gitem, np. `evals/corpus/.work`).
    pub work: PathBuf,
}

impl WhisperCli {
    /// Oś słów pliku (opcjonalnie segmentu).
    pub fn file_words(&self, wav: &Path, segment: Option<Segment>) -> Result<Vec<Word>, String> {
        whisper_words(&self.exe, &self.model, wav, &self.lang, segment)
    }

    /// Oś słów dla próbek 16 kHz (plik tymczasowy w katalogu roboczym, usuwany po odczycie).
    pub fn pcm_words(&self, pcm: &[f32], tag: usize) -> Result<Vec<Word>, String> {
        std::fs::create_dir_all(&self.work).map_err(|e| format!("{}: {e}", self.work.display()))?;
        let path = self
            .work
            .join(format!("prefix-{}-{tag}.wav", std::process::id()));
        std::fs::write(&path, wav16(pcm)).map_err(|e| format!("{}: {e}", path.display()))?;
        let words = self.file_words(&path, None);
        let _ = std::fs::remove_file(&path);
        words
    }
}

#[derive(Debug, Default)]
struct State {
    active: Option<UtteranceId>,
    pcm: Vec<f32>,
}

/// STT „partial = model na audio do teraz” (jedna wypowiedź naraz).
#[derive(Debug)]
pub struct PrefixStt {
    cli: WhisperCli,
    state: Mutex<State>,
    calls: AtomicUsize,
}

impl PrefixStt {
    /// Nowy adapter.
    pub fn new(cli: WhisperCli) -> Self {
        Self {
            cli,
            state: Mutex::new(State::default()),
            calls: AtomicUsize::new(0),
        }
    }

    /// Liczba wywołań modelu (koszt przebiegu).
    pub fn calls(&self) -> usize {
        self.calls.load(Ordering::Relaxed)
    }

    fn lock(&self) -> Result<std::sync::MutexGuard<'_, State>, SttError> {
        self.state
            .lock()
            .map_err(|_| SttError::NotAvailable("zatruta blokada".into()))
    }

    fn snapshot(&self, id: UtteranceId, end: bool) -> Result<Vec<f32>, SttError> {
        let mut s = self.lock()?;
        if s.active != Some(id) {
            return Err(SttError::UnknownUtterance(id));
        }
        Ok(if end {
            std::mem::take(&mut *s).pcm
        } else {
            s.pcm.clone()
        })
    }

    fn transcribe(
        &self,
        id: UtteranceId,
        pcm: &[f32],
        is_final: bool,
    ) -> Result<Transcript, SttError> {
        let tag = self.calls.fetch_add(1, Ordering::Relaxed);
        let words = self.cli.pcm_words(pcm, tag).map_err(SttError::Sidecar)?;
        let text = words
            .iter()
            .map(|w| w.text.as_str())
            .collect::<Vec<_>>()
            .join(" ");
        Ok(Transcript {
            utterance: id,
            text,
            confidence: if words.is_empty() { 0.0 } else { 1.0 },
            words,
            lang: self.cli.lang.clone(),
            is_final,
            latency_ms: 0,
            backend: None,
        })
    }
}

#[async_trait]
impl Stt for PrefixStt {
    async fn configure(&self, _cfg: SttCfg) -> Result<(), SttError> {
        Ok(())
    }

    async fn start_utterance(&self, id: UtteranceId) -> Result<(), SttError> {
        let mut s = self.lock()?;
        if s.active.is_some() {
            return Err(SttError::DuplicateUtterance(id));
        }
        *s = State {
            active: Some(id),
            pcm: Vec::new(),
        };
        Ok(())
    }

    async fn push(&self, id: UtteranceId, frame: &Frame) -> Result<Option<Transcript>, SttError> {
        if frame.format.sample_rate != PIPELINE_RATE || frame.format.channels != 1 {
            return Err(SttError::Format("wymagane mono 16 kHz".into()));
        }
        let mut s = self.lock()?;
        if s.active != Some(id) {
            return Err(SttError::UnknownUtterance(id));
        }
        s.pcm.extend_from_slice(&frame.pcm);
        Ok(None)
    }

    async fn partial_now(&self, id: UtteranceId) -> Result<Option<Transcript>, SttError> {
        let pcm = self.snapshot(id, false)?;
        let t = self.transcribe(id, &pcm, false)?;
        Ok((!t.words.is_empty()).then_some(t))
    }

    async fn end_utterance(&self, id: UtteranceId) -> Result<Transcript, SttError> {
        let pcm = self.snapshot(id, true)?;
        self.transcribe(id, &pcm, true)
    }

    async fn cancel(&self, id: UtteranceId) {
        let _ = self.snapshot(id, true);
    }

    fn health(&self) -> Health {
        Health::Ready(Backend::Cpu)
    }

    fn take_events(&self) -> Vec<SttEvent> {
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use voice_audio_contract::MediaTime;

    use super::*;

    #[tokio::test]
    async fn buffers_audio_and_reports_missing_cli() {
        let work = std::env::temp_dir().join(format!("alfa-f2-prefix-{}", std::process::id()));
        let cli = WhisperCli {
            exe: "/nie/ma/whisper-cli".into(),
            model: "m.bin".into(),
            lang: "pl".into(),
            work: work.clone(),
        };
        let stt = PrefixStt::new(cli);
        let id = UtteranceId(7);
        stt.start_utterance(id).await.unwrap();
        let frame = Frame::mono(vec![0.1; 160], PIPELINE_RATE, MediaTime::from_ms(0));
        stt.push(id, &frame).await.unwrap();
        let stereo = Frame::mono(vec![0.1; 480], 48_000, MediaTime::from_ms(0));
        assert!(matches!(
            stt.push(id, &stereo).await,
            Err(SttError::Format(_))
        ));
        let err = stt.partial_now(id).await.unwrap_err();
        assert!(
            matches!(err, SttError::Sidecar(ref m) if m.starts_with("whisper-cli:")),
            "{err:?}"
        );
        assert_eq!(stt.calls(), 1);
        assert!(
            std::fs::read_dir(&work).unwrap().next().is_none(),
            "prefiks usunięty"
        );
        assert!(stt.end_utterance(id).await.is_err());
        assert_eq!(
            stt.partial_now(id).await,
            Err(SttError::UnknownUtterance(id))
        );
        stt.cancel(id).await;
        assert!(stt.take_events().is_empty() && stt.configure(SttCfg::default()).await.is_ok());
        assert_eq!(stt.health(), Health::Ready(Backend::Cpu));
        std::fs::remove_dir_all(&work).unwrap();
    }
}
