//! Protokół JSON-lines po stdio — ten sam, którym `voice-tts-impl` rozmawia z sidecarami mowy:
//! żądanie `{id, op:"synth", text, voice, …}` → jedna albo więcej linii `audio` (PCM16 base64),
//! potem `done`; błąd to linia `error`. Silnik jest za cechą [`Synth`], więc protokół ma testy
//! bez modelu.

use std::io::{BufRead, Write};

use base64::Engine as _;
use serde::{Deserialize, Serialize};

/// Ustawienia jednej syntezy.
#[derive(Debug, Clone, PartialEq)]
pub struct SynthParams {
    /// Barwa głosu (`F1`…`F5`, `M1`…`M5`).
    pub voice: String,
    /// Język tekstu (`pl`, `en`, …).
    pub lang: String,
    /// Liczba kroków odszumiania (więcej = lepiej i wolniej).
    pub steps: usize,
    /// Tempo mowy (1.0 = naturalne).
    pub speed: f32,
}

/// Silnik mowy: zdanie → próbki mono i częstotliwość.
pub trait Synth {
    /// Synteza jednego zdania.
    fn synth(&mut self, text: &str, params: &SynthParams) -> Result<(Vec<f32>, u32), String>;
}

/// Wartości domyślne procesu (z argumentów uruchomienia).
#[derive(Debug, Clone, PartialEq)]
pub struct Defaults {
    /// Język, gdy żądanie go nie podaje.
    pub lang: String,
    /// Kroki, gdy żądanie ich nie podaje (Delta: 6).
    pub steps: usize,
    /// Tempo, gdy żądanie go nie podaje (Delta: 1.05).
    pub speed: f32,
}

impl Default for Defaults {
    fn default() -> Self {
        Self {
            lang: "pl".into(),
            steps: 6,
            speed: 1.05,
        }
    }
}

#[derive(Debug, Deserialize)]
struct Request {
    id: u64,
    op: String,
    #[serde(default)]
    text: String,
    #[serde(default)]
    voice: String,
    #[serde(default)]
    lang: Option<String>,
    #[serde(default)]
    steps: Option<usize>,
    /// Tempo docelowe; `voice-tts-impl` wysyła je jako `rate`.
    #[serde(default)]
    rate: Option<f32>,
}

#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum Reply<'a> {
    Audio {
        id: u64,
        pcm16: String,
        sample_rate: u32,
    },
    Done {
        id: u64,
    },
    Error {
        id: u64,
        message: &'a str,
    },
}

/// Próbki `f32` → PCM16 LE z ograniczeniem do zakresu.
pub fn f32_to_pcm16(samples: &[f32]) -> Vec<u8> {
    let mut out = Vec::with_capacity(samples.len() * 2);
    for s in samples {
        #[allow(clippy::cast_possible_truncation)]
        let v = (s.clamp(-1.0, 1.0) * 32_767.0).round() as i16;
        out.extend_from_slice(&v.to_le_bytes());
    }
    out
}

/// Najwięcej próbek w jednej linii `audio` (ok. 1 s przy 44,1 kHz) — krótsze linie, szybszy odbiór.
const CHUNK_SAMPLES: usize = 44_100;

fn write_line(out: &mut impl Write, reply: &Reply<'_>) -> std::io::Result<()> {
    let line = serde_json::to_string(reply).map_err(std::io::Error::other)?;
    out.write_all(line.as_bytes())?;
    out.write_all(b"\n")?;
    out.flush()
}

/// Obsługa jednej linii żądania. Zwraca `false`, gdy proces ma się zakończyć (`op: "quit"`).
pub fn handle_line(
    line: &str,
    engine: &mut impl Synth,
    defaults: &Defaults,
    out: &mut impl Write,
) -> std::io::Result<bool> {
    let line = line.trim();
    if line.is_empty() {
        return Ok(true);
    }
    let Ok(req) = serde_json::from_str::<Request>(line) else {
        return Ok(true); // nie-JSON pomijamy, jak druga strona pomija śmieci na stdout
    };
    match req.op.as_str() {
        "quit" => return Ok(false),
        "ping" => write_line(out, &Reply::Done { id: req.id })?,
        "synth" => {
            let params = SynthParams {
                voice: if req.voice.is_empty() {
                    "F1".into()
                } else {
                    req.voice
                },
                lang: req.lang.unwrap_or_else(|| defaults.lang.clone()),
                steps: req.steps.unwrap_or(defaults.steps).clamp(1, 32),
                speed: req.rate.unwrap_or(defaults.speed).clamp(0.5, 2.0),
            };
            match engine.synth(&req.text, &params) {
                Ok((samples, rate)) if !samples.is_empty() => {
                    for chunk in samples.chunks(CHUNK_SAMPLES) {
                        let pcm16 =
                            base64::engine::general_purpose::STANDARD.encode(f32_to_pcm16(chunk));
                        write_line(
                            out,
                            &Reply::Audio {
                                id: req.id,
                                pcm16,
                                sample_rate: rate,
                            },
                        )?;
                    }
                    write_line(out, &Reply::Done { id: req.id })?;
                }
                Ok(_) => write_line(
                    out,
                    &Reply::Error {
                        id: req.id,
                        message: "silnik nie zwrócił dźwięku",
                    },
                )?,
                Err(message) => write_line(
                    out,
                    &Reply::Error {
                        id: req.id,
                        message: &message,
                    },
                )?,
            }
        }
        other => write_line(
            out,
            &Reply::Error {
                id: req.id,
                message: &format!("nieznana operacja: {other}"),
            },
        )?,
    }
    Ok(true)
}

/// Pętla procesu: linie z `input` aż do końca wejścia albo `quit`.
pub fn serve(
    input: impl BufRead,
    engine: &mut impl Synth,
    defaults: &Defaults,
    out: &mut impl Write,
) -> std::io::Result<()> {
    for line in input.lines() {
        if !handle_line(&line?, engine, defaults, out)? {
            break;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Fake {
        seen: Vec<(String, SynthParams)>,
        samples: usize,
    }

    impl Synth for Fake {
        fn synth(&mut self, text: &str, params: &SynthParams) -> Result<(Vec<f32>, u32), String> {
            self.seen.push((text.to_owned(), params.clone()));
            if text == "błąd" {
                return Err("zepsute".into());
            }
            Ok((vec![0.5; self.samples], 44_100))
        }
    }

    fn run(input: &str, samples: usize) -> (Vec<serde_json::Value>, Fake) {
        let mut engine = Fake {
            seen: Vec::new(),
            samples,
        };
        let mut out = Vec::new();
        let served = serve(
            input.as_bytes(),
            &mut engine,
            &Defaults::default(),
            &mut out,
        );
        assert!(served.is_ok());
        let lines = String::from_utf8_lossy(&out)
            .lines()
            .filter_map(|l| serde_json::from_str(l).ok())
            .collect();
        (lines, engine)
    }

    #[test]
    fn synth_uses_defaults_and_ends_with_done() {
        let (lines, engine) = run(
            "{\"id\":1,\"op\":\"synth\",\"text\":\"Cześć\",\"voice\":\"\"}\n",
            10,
        );
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0]["type"], "audio");
        assert_eq!(lines[0]["sample_rate"], 44_100);
        assert_eq!(lines[1]["type"], "done");
        let params = &engine.seen[0].1;
        assert_eq!((params.voice.as_str(), params.lang.as_str()), ("F1", "pl"));
        assert_eq!(params.steps, 6);
        assert!((params.speed - 1.05).abs() < 1e-6);
    }

    #[test]
    fn request_overrides_and_limits() {
        let (_, engine) = run(
            "{\"id\":2,\"op\":\"synth\",\"text\":\"a\",\"voice\":\"M3\",\"lang\":\"en\",\"steps\":99,\"rate\":9.0}\n",
            4,
        );
        let params = &engine.seen[0].1;
        assert_eq!((params.voice.as_str(), params.lang.as_str()), ("M3", "en"));
        assert_eq!(params.steps, 32);
        assert!((params.speed - 2.0).abs() < 1e-6);
    }

    #[test]
    fn long_audio_is_split_into_lines() {
        let (lines, _) = run(
            "{\"id\":3,\"op\":\"synth\",\"text\":\"a\",\"voice\":\"F2\"}\n",
            100_000,
        );
        let audio = lines.iter().filter(|l| l["type"] == "audio").count();
        assert_eq!(audio, 3);
        assert_eq!(lines.last().map(|l| l["type"].clone()), Some("done".into()));
    }

    #[test]
    fn errors_garbage_ping_and_quit() {
        let input = "śmieci\n{\"id\":4,\"op\":\"synth\",\"text\":\"błąd\",\"voice\":\"F1\"}\n\
                     {\"id\":5,\"op\":\"ping\"}\n{\"id\":6,\"op\":\"nic\"}\n{\"id\":7,\"op\":\"quit\"}\n\
                     {\"id\":8,\"op\":\"ping\"}\n";
        let (lines, _) = run(input, 4);
        assert_eq!(lines.len(), 3);
        assert_eq!(
            (lines[0]["type"].as_str(), lines[0]["message"].as_str()),
            (Some("error"), Some("zepsute"))
        );
        assert_eq!(
            (lines[1]["type"].as_str(), lines[1]["id"].as_u64()),
            (Some("done"), Some(5))
        );
        assert_eq!(lines[2]["type"], "error");
    }

    #[test]
    fn pcm16_clamps() {
        assert_eq!(
            f32_to_pcm16(&[0.0, 2.0, -2.0]),
            vec![0, 0, 0xFF, 0x7F, 0x01, 0x80]
        );
    }
}
