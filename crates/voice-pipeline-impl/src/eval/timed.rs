//! STT „z osią słów” dla runnera offline. Prawdziwy model (whisper.cpp przez `whisper-cli`,
//! segmenty jednowyrazowe ze znacznikami czasu) transkrybuje całą pozycję raz; [`TimedStt`]
//! odtwarza z tej osi strumień: partial w chwili `t` = słowa zakończone do `t`, final = wszystkie.
//! Wynik jest optymistyczny wobec prawdziwego strumienia (model widzi cały plik) — opóźnienie
//! obliczeń mierzy Voice Lab na żywym potoku; tu liczy się czas akustyczny (kadencja partiali).

use std::path::Path;
use std::process::Command;
use std::sync::Mutex;

use async_trait::async_trait;
use device_profile_contract::Backend;
use voice_audio_contract::Frame;
use voice_stt_contract::{Health, Stt, SttCfg, SttError, SttEvent, Transcript, UtteranceId, Word};

use crate::eval::manifest::Segment;

#[derive(Debug, Default)]
struct State {
    active: Option<UtteranceId>,
    pushed_ms: u64,
}

/// Strumień STT odtwarzany z gotowej osi słów (jedna wypowiedź naraz).
#[derive(Debug)]
pub struct TimedStt {
    words: Vec<Word>,
    lang: String,
    state: Mutex<State>,
}

impl TimedStt {
    /// Oś słów (czasy względem początku pozycji).
    pub fn new(words: Vec<Word>, lang: &str) -> Self {
        Self {
            words,
            lang: lang.to_owned(),
            state: Mutex::new(State::default()),
        }
    }

    fn with_state<T>(
        &self,
        id: UtteranceId,
        f: impl FnOnce(&mut State) -> T,
    ) -> Result<T, SttError> {
        let mut s = self
            .state
            .lock()
            .map_err(|_| SttError::NotAvailable("zatruta blokada".into()))?;
        if s.active != Some(id) {
            return Err(SttError::UnknownUtterance(id));
        }
        Ok(f(&mut s))
    }

    fn transcript(&self, id: UtteranceId, until_ms: Option<u64>) -> Transcript {
        let words: Vec<Word> = self
            .words
            .iter()
            .filter(|w| until_ms.is_none_or(|t| u64::from(w.end_ms) <= t))
            .cloned()
            .collect();
        let text = words
            .iter()
            .map(|w| w.text.as_str())
            .collect::<Vec<_>>()
            .join(" ");
        let confidence = if words.is_empty() {
            0.0
        } else {
            words.iter().map(|w| w.confidence).sum::<f32>() / words.len() as f32
        };
        Transcript {
            utterance: id,
            text,
            words,
            lang: self.lang.clone(),
            is_final: until_ms.is_none(),
            confidence,
            latency_ms: 0,
            backend: Some(Backend::Cpu),
        }
    }
}

#[async_trait]
impl Stt for TimedStt {
    async fn configure(&self, _cfg: SttCfg) -> Result<(), SttError> {
        Ok(())
    }

    async fn start_utterance(&self, id: UtteranceId) -> Result<(), SttError> {
        let mut s = self
            .state
            .lock()
            .map_err(|_| SttError::NotAvailable("zatruta blokada".into()))?;
        if s.active.is_some() {
            return Err(SttError::DuplicateUtterance(id));
        }
        *s = State {
            active: Some(id),
            pushed_ms: 0,
        };
        Ok(())
    }

    async fn push(&self, id: UtteranceId, frame: &Frame) -> Result<Option<Transcript>, SttError> {
        let ms = u64::try_from(frame.duration().as_millis()).unwrap_or(u64::MAX);
        self.with_state(id, |s| s.pushed_ms = s.pushed_ms.saturating_add(ms))?;
        Ok(None)
    }

    async fn partial_now(&self, id: UtteranceId) -> Result<Option<Transcript>, SttError> {
        let until = self.with_state(id, |s| s.pushed_ms)?;
        let t = self.transcript(id, Some(until));
        Ok((!t.words.is_empty()).then_some(t))
    }

    async fn end_utterance(&self, id: UtteranceId) -> Result<Transcript, SttError> {
        self.with_state(id, |s| *s = State::default())?;
        Ok(self.transcript(id, None))
    }

    async fn cancel(&self, id: UtteranceId) {
        let _ = self.with_state(id, |s| *s = State::default());
    }

    fn health(&self) -> Health {
        Health::Ready(Backend::Cpu)
    }

    fn take_events(&self) -> Vec<SttEvent> {
        Vec::new()
    }
}

fn parse_ts(ts: &str) -> Option<u32> {
    let (hms, ms) = ts.trim().split_once('.')?;
    let mut parts = hms.split(':').map(str::parse::<u32>);
    let (h, m, s) = (
        parts.next()?.ok()?,
        parts.next()?.ok()?,
        parts.next()?.ok()?,
    );
    Some(((h * 60 + m) * 60 + s) * 1_000 + ms.parse::<u32>().ok()?)
}

/// Oś słów z wyjścia `whisper-cli -ml 1 -sow` (linie `[00:00:01.240 --> 00:00:01.560]  słowo`);
/// czasy pomniejszone o `offset_ms` (początek segmentu).
pub fn parse_whisper_words(stdout: &str, offset_ms: u64) -> Vec<Word> {
    let off = u32::try_from(offset_ms).unwrap_or(u32::MAX);
    stdout
        .lines()
        .filter_map(|line| {
            let rest = line.trim().strip_prefix('[')?;
            let (span, text) = rest.split_once(']')?;
            let (a, b) = span.split_once("-->")?;
            let text = text.trim();
            (!text.is_empty()).then(|| Word {
                text: text.to_owned(),
                start_ms: parse_ts(a).unwrap_or(0).saturating_sub(off),
                end_ms: parse_ts(b).unwrap_or(0).saturating_sub(off),
                confidence: 1.0,
            })
        })
        .collect()
}

/// Transkrypcja pozycji przez `whisper-cli` (whisper.cpp) z osią słów; `segment` → `-ot`/`-d`.
/// Plik musi być WAV 16 kHz mono (katalog `16k/` korpusu).
pub fn whisper_words(
    exe: &Path,
    model: &Path,
    wav: &Path,
    lang: &str,
    segment: Option<Segment>,
) -> Result<Vec<Word>, String> {
    let mut cmd = Command::new(exe);
    cmd.arg("-m")
        .arg(model)
        .arg("-f")
        .arg(wav)
        .args(["-l", lang, "-ml", "1", "-sow", "-np"]);
    if let Some(s) = segment {
        cmd.args([
            "-ot",
            &s.start_ms.to_string(),
            "-d",
            &s.end_ms.saturating_sub(s.start_ms).to_string(),
        ]);
    }
    let out = cmd.output().map_err(|e| format!("whisper-cli: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "whisper-cli zakończył się błędem: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    Ok(parse_whisper_words(
        &String::from_utf8_lossy(&out.stdout),
        segment.map_or(0, |s| s.start_ms),
    ))
}

#[cfg(test)]
mod tests {
    use voice_audio_contract::{MediaTime, PIPELINE_RATE};

    use super::*;

    const OUT: &str = "\
[00:00:00.000 --> 00:00:00.320]   Stop
[00:00:00.320 --> 00:00:00.330]
[00:00:01.000 --> 00:00:01.480]   proszę
garbage line
";

    #[test]
    fn parses_word_timeline_with_offset() {
        let w = parse_whisper_words(OUT, 0);
        assert_eq!(w.len(), 2);
        assert_eq!(
            (w[0].text.as_str(), w[0].start_ms, w[0].end_ms),
            ("Stop", 0, 320)
        );
        assert_eq!((w[1].start_ms, w[1].end_ms), (1_000, 1_480));
        let shifted = parse_whisper_words(OUT, 500);
        assert_eq!((shifted[0].end_ms, shifted[1].start_ms), (0, 500));
        assert_eq!(parse_ts("01:02:03.004"), Some(3_723_004));
        assert_eq!(parse_ts("bez"), None);
    }

    #[tokio::test]
    async fn streams_partials_from_timeline() {
        let stt = TimedStt::new(parse_whisper_words(OUT, 0), "pl");
        let id = UtteranceId(1);
        assert_eq!(
            stt.partial_now(id).await,
            Err(SttError::UnknownUtterance(id))
        );
        stt.start_utterance(id).await.unwrap();
        assert_eq!(
            stt.start_utterance(UtteranceId(2)).await,
            Err(SttError::DuplicateUtterance(UtteranceId(2)))
        );
        let frame =
            |i: u64| Frame::mono(vec![0.0; 1_600], PIPELINE_RATE, MediaTime::from_ms(i * 100));
        assert_eq!(stt.partial_now(id).await.unwrap(), None);
        for i in 0..4 {
            stt.push(id, &frame(i)).await.unwrap();
        }
        let p = stt.partial_now(id).await.unwrap().unwrap();
        assert_eq!((p.text.as_str(), p.is_final), ("Stop", false));
        let f = stt.end_utterance(id).await.unwrap();
        assert_eq!(
            (f.text.as_str(), f.is_final, f.confidence),
            ("Stop proszę", true, 1.0)
        );
        stt.cancel(id).await;
        assert_eq!(stt.health(), Health::Ready(Backend::Cpu));
        assert!(stt.take_events().is_empty() && stt.configure(SttCfg::default()).await.is_ok());
    }

    #[test]
    fn missing_whisper_cli_is_an_error() {
        let r = whisper_words(
            Path::new("/nie/ma/whisper-cli"),
            Path::new("m.bin"),
            Path::new("a.wav"),
            "pl",
            None,
        );
        assert!(r.unwrap_err().starts_with("whisper-cli:"));
    }
}
