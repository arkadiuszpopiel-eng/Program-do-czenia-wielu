//! Klient HTTP `whisper-server` (127.0.0.1, bez proxy): `/health`, `/inference` (multipart, WAV
//! PCM16 16 kHz) i parser `verbose_json` (tokeny → słowa, pewność, język).

use std::time::{Duration, Instant};

use device_profile_contract::Backend;
use reqwest::multipart::{Form, Part};
use serde::Deserialize;
use voice_audio_contract::AudioFormat;
use voice_audio_contract::wav::{WavEncoding, encode_wav};
use voice_stt_contract::{SttError, Transcript, UtteranceId, Word};

/// Parametry jednego rozpoznania.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InferenceParams {
    /// Kod języka (`auto`, `pl`, `en`).
    pub language: String,
    /// Wiązka (1 = zachłannie).
    pub beam_size: u8,
    /// Prompt początkowy (hotwords).
    pub prompt: Option<String>,
}

#[derive(Debug, Deserialize)]
struct VerboseJson {
    #[serde(default)]
    text: String,
    #[serde(default)]
    language: Option<String>,
    #[serde(default)]
    segments: Vec<Segment>,
}

#[derive(Debug, Deserialize)]
struct Segment {
    #[serde(default)]
    words: Vec<Token>,
    #[serde(default)]
    avg_logprob: Option<f64>,
}

#[derive(Debug, Deserialize)]
struct Token {
    word: String,
    #[serde(default)]
    start: Option<f64>,
    #[serde(default)]
    end: Option<f64>,
    #[serde(default)]
    probability: Option<f64>,
}

/// Klient serwera.
#[derive(Debug, Clone)]
pub struct WhisperClient {
    http: reqwest::Client,
}

fn err(e: impl std::fmt::Display) -> SttError {
    SttError::Sidecar(e.to_string())
}

impl WhisperClient {
    /// Klient bez proxy (ruch wyłącznie na 127.0.0.1).
    pub fn new(timeout: Duration) -> Result<Self, SttError> {
        let http = reqwest::Client::builder()
            .no_proxy()
            .timeout(timeout)
            .connect_timeout(Duration::from_secs(2))
            .build()
            .map_err(err)?;
        Ok(Self { http })
    }

    /// `true`, gdy serwer odpowiada 200 na `/health` (503 = model się ładuje).
    pub async fn healthy(&self, base: &str) -> bool {
        matches!(self.http.get(format!("{base}/health")).send().await, Ok(r) if r.status().is_success())
    }

    /// Rozpoznaje próbki 16 kHz mono.
    pub async fn inference(
        &self,
        base: &str,
        samples: &[f32],
        params: &InferenceParams,
        utterance: UtteranceId,
        backend: Backend,
        is_final: bool,
    ) -> Result<Transcript, reqwest::Error> {
        let wav = encode_wav(samples, AudioFormat::mono(16_000), WavEncoding::Pcm16);
        let file = Part::bytes(wav)
            .file_name("utterance.wav")
            .mime_str("audio/wav")?;
        let mut form = Form::new()
            .part("file", file)
            .text("response_format", "verbose_json")
            .text("temperature", "0.0")
            .text("token_timestamps", "true")
            .text("suppress_non_speech", "true")
            .text("language", params.language.clone())
            .text("beam_size", params.beam_size.to_string());
        if let Some(p) = &params.prompt {
            form = form.text("prompt", p.clone());
        }
        let started = Instant::now();
        let body = self
            .http
            .post(format!("{base}/inference"))
            .multipart(form)
            .send()
            .await?
            .error_for_status()?
            .text()
            .await?;
        let latency = u32::try_from(started.elapsed().as_millis()).unwrap_or(u32::MAX);
        Ok(parse_verbose_json(
            &body, utterance, backend, is_final, latency,
        ))
    }
}

/// Pełna nazwa języka whisper → ISO 639-1.
pub fn iso_lang(name: &str) -> String {
    let n = name.trim().to_lowercase();
    match n.as_str() {
        "polish" => "pl".into(),
        "english" => "en".into(),
        "german" => "de".into(),
        "ukrainian" => "uk".into(),
        "russian" => "ru".into(),
        "czech" => "cs".into(),
        "french" => "fr".into(),
        "spanish" => "es".into(),
        other if other.len() == 2 => other.into(),
        other => other.chars().take(2).collect(),
    }
}

/// Token whisper: (tekst, początek s, koniec s, prawdopodobieństwo).
pub type RawToken = (String, Option<f64>, Option<f64>, Option<f64>);

/// Łączy tokeny whisper w słowa (token zaczynający się spacją otwiera słowo; tokeny specjalne pomijane).
pub fn merge_tokens(segments: &[RawToken]) -> Vec<Word> {
    let mut words: Vec<(Word, u32)> = Vec::new();
    for (text, start, end, p) in segments {
        if text.starts_with("[_") || text.starts_with("<|") || text.is_empty() {
            continue;
        }
        let ms = |t: Option<f64>| t.map_or(0, |s| (s.max(0.0) * 1000.0).round() as u32);
        let conf = p.unwrap_or(0.5).clamp(0.0, 1.0) as f32;
        let starts_word = text.starts_with(' ') || words.is_empty();
        if starts_word {
            words.push((
                Word {
                    text: text.trim().to_owned(),
                    start_ms: ms(*start),
                    end_ms: ms(*end),
                    confidence: conf,
                },
                1,
            ));
        } else if let Some((w, n)) = words.last_mut() {
            w.text.push_str(text.trim_end());
            w.end_ms = w.end_ms.max(ms(*end));
            w.confidence = (w.confidence * *n as f32 + conf) / (*n + 1) as f32;
            *n += 1;
        }
    }
    words
        .into_iter()
        .map(|(w, _)| w)
        .filter(|w| !w.text.is_empty())
        .collect()
}

/// Parsuje odpowiedź `verbose_json` (przy błędzie formatu — tekst bez słów, pewność 0).
pub fn parse_verbose_json(
    body: &str,
    utterance: UtteranceId,
    backend: Backend,
    is_final: bool,
    latency_ms: u32,
) -> Transcript {
    let parsed: Option<VerboseJson> = serde_json::from_str(body).ok();
    let (text, lang, words, logprob) = match &parsed {
        Some(v) => {
            let tokens: Vec<_> = v
                .segments
                .iter()
                .flat_map(|s| {
                    s.words
                        .iter()
                        .map(|t| (t.word.clone(), t.start, t.end, t.probability))
                })
                .collect();
            let lp: Vec<f64> = v.segments.iter().filter_map(|s| s.avg_logprob).collect();
            (
                v.text.trim().to_owned(),
                v.language.as_deref().map(iso_lang).unwrap_or_default(),
                merge_tokens(&tokens),
                lp,
            )
        }
        None => (String::new(), String::new(), Vec::new(), Vec::new()),
    };
    let confidence = if !words.is_empty() {
        words.iter().map(|w| w.confidence).sum::<f32>() / words.len() as f32
    } else if !logprob.is_empty() {
        (logprob.iter().sum::<f64>() / logprob.len() as f64)
            .exp()
            .clamp(0.0, 1.0) as f32
    } else {
        0.0
    };
    Transcript {
        utterance,
        text,
        words,
        lang,
        is_final,
        confidence,
        latency_ms,
        backend: Some(backend),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_whisper_server_verbose_json() {
        let body = r#"{"task":"transcribe","language":"polish","duration":2.0,"text":" Delta, otwórz plik.",
            "segments":[{"id":0,"text":" Delta, otwórz plik.","start":0.0,"end":2.0,
            "words":[{"word":"[_BEG_]","start":0.0,"end":0.0,"probability":0.9},
                     {"word":" Del","start":0.1,"end":0.3,"probability":0.9},{"word":"ta,","start":0.3,"end":0.5,"probability":0.7},
                     {"word":" otw","start":0.6,"end":0.8,"probability":0.95},{"word":"órz","start":0.8,"end":1.0,"probability":0.85},
                     {"word":" plik.","start":1.1,"end":1.5,"probability":0.99}],
            "temperature":0.0,"avg_logprob":-0.2,"no_speech_prob":0.01}]}"#;
        let t = parse_verbose_json(body, UtteranceId(1), Backend::Vulkan, true, 120);
        assert_eq!(t.text, "Delta, otwórz plik.");
        assert_eq!(t.lang, "pl");
        let w: Vec<&str> = t.words.iter().map(|w| w.text.as_str()).collect();
        assert_eq!(w, ["Delta,", "otwórz", "plik."]);
        assert_eq!((t.words[0].start_ms, t.words[0].end_ms), (100, 500));
        assert!((t.words[0].confidence - 0.8).abs() < 1e-5);
        assert!((t.confidence - (0.8 + 0.9 + 0.99) / 3.0).abs() < 1e-4);
        assert_eq!(t.latency_ms, 120);
    }

    #[test]
    fn tolerates_minimal_and_broken_responses() {
        let t = parse_verbose_json(
            r#"{"text":" hello","language":"english","segments":[{"avg_logprob":-0.1}]}"#,
            UtteranceId(2),
            Backend::Cpu,
            false,
            0,
        );
        assert_eq!((t.text.as_str(), t.lang.as_str()), ("hello", "en"));
        assert!((t.confidence - (-0.1f64).exp() as f32).abs() < 1e-5);
        let bad = parse_verbose_json("<html>", UtteranceId(3), Backend::Cpu, true, 0);
        assert!(bad.text.is_empty() && bad.confidence == 0.0);
        assert_eq!(iso_lang("pl"), "pl");
        assert_eq!(iso_lang("Czech"), "cs");
        assert_eq!(iso_lang("swahili"), "sw");
        assert!(WhisperClient::new(Duration::from_secs(1)).is_ok());
    }
}
