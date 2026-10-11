//! Protokół JSON-lines po stdio: żądanie `{id, op:"score", pcm16}` (nagranie 16 kHz mono, PCM16
//! w base64) → `{type:"score", id, probability, ms}`; błąd to linia `error`. Model jest za cechą
//! [`Scorer`], więc protokół ma testy bez modelu.

use std::io::{BufRead, Write};
use std::time::Instant;

use base64::Engine as _;
use serde::{Deserialize, Serialize};

/// Ocena nagrania: prawdopodobieństwo, że rozmówca skończył (0–1).
pub trait Scorer {
    /// Ocena próbek 16 kHz mono.
    fn score(&mut self, samples: &[f32]) -> Result<f32, String>;
}

#[derive(Debug, Deserialize)]
struct Request {
    id: u64,
    op: String,
    #[serde(default)]
    pcm16: String,
}

#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum Reply<'a> {
    Score { id: u64, probability: f32, ms: u64 },
    Done { id: u64 },
    Error { id: u64, message: &'a str },
}

/// PCM16 LE → próbki `f32` (niepełna ostatnia próbka jest pomijana).
pub fn pcm16_to_f32(bytes: &[u8]) -> Vec<f32> {
    bytes
        .chunks_exact(2)
        .map(|c| f32::from(i16::from_le_bytes([c[0], c[1]])) / 32_768.0)
        .collect()
}

fn write_line(out: &mut impl Write, reply: &Reply<'_>) -> std::io::Result<()> {
    let line = serde_json::to_string(reply).map_err(std::io::Error::other)?;
    out.write_all(line.as_bytes())?;
    out.write_all(b"\n")?;
    out.flush()
}

/// Obsługa jednej linii żądania. Zwraca `false`, gdy proces ma się zakończyć (`op: "quit"`).
pub fn handle_line(
    line: &str,
    scorer: &mut impl Scorer,
    out: &mut impl Write,
) -> std::io::Result<bool> {
    let line = line.trim();
    if line.is_empty() {
        return Ok(true);
    }
    let Ok(req) = serde_json::from_str::<Request>(line) else {
        return Ok(true); // nie-JSON pomijamy
    };
    let id = req.id;
    match req.op.as_str() {
        "quit" => return Ok(false),
        "ping" => write_line(out, &Reply::Done { id })?,
        "score" => {
            let started = Instant::now();
            let result = base64::engine::general_purpose::STANDARD
                .decode(req.pcm16.as_bytes())
                .map_err(|e| format!("niepoprawne nagranie: {e}"))
                .and_then(|bytes| scorer.score(&pcm16_to_f32(&bytes)));
            match result {
                Ok(p) if p.is_finite() => write_line(
                    out,
                    &Reply::Score {
                        id,
                        probability: p.clamp(0.0, 1.0),
                        ms: u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
                    },
                )?,
                Ok(_) => write_line(
                    out,
                    &Reply::Error {
                        id,
                        message: "model zwrócił wartość nieliczbową",
                    },
                )?,
                Err(message) => write_line(
                    out,
                    &Reply::Error {
                        id,
                        message: &message,
                    },
                )?,
            }
        }
        other => write_line(
            out,
            &Reply::Error {
                id,
                message: &format!("nieznana operacja: {other}"),
            },
        )?,
    }
    Ok(true)
}

/// Pętla procesu: linie z `input` aż do końca wejścia albo `quit`.
pub fn serve(
    input: impl BufRead,
    scorer: &mut impl Scorer,
    out: &mut impl Write,
) -> std::io::Result<()> {
    for line in input.lines() {
        if !handle_line(&line?, scorer, out)? {
            break;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Fake(Vec<usize>);

    impl Scorer for Fake {
        fn score(&mut self, samples: &[f32]) -> Result<f32, String> {
            self.0.push(samples.len());
            match samples.len() {
                0 => Err("puste nagranie".into()),
                1 => Ok(f32::NAN),
                2 => Ok(7.0),
                _ => Ok(0.25),
            }
        }
    }

    fn pcm(n: usize) -> String {
        base64::engine::general_purpose::STANDARD.encode(vec![0u8; n * 2])
    }

    fn run(input: &str) -> (Vec<serde_json::Value>, Vec<usize>) {
        let mut fake = Fake(Vec::new());
        let mut out = Vec::new();
        assert!(serve(input.as_bytes(), &mut fake, &mut out).is_ok());
        let lines = String::from_utf8_lossy(&out)
            .lines()
            .filter_map(|l| serde_json::from_str(l).ok())
            .collect();
        (lines, fake.0)
    }

    #[test]
    fn score_returns_probability_and_time() {
        let (lines, seen) = run(&format!(
            "{{\"id\":1,\"op\":\"score\",\"pcm16\":\"{}\"}}\n",
            pcm(100)
        ));
        assert_eq!(seen, vec![100]);
        assert_eq!(lines[0]["type"], "score");
        assert_eq!(lines[0]["id"], 1);
        assert!((lines[0]["probability"].as_f64().unwrap() - 0.25).abs() < 1e-6);
        assert!(lines[0]["ms"].is_u64());
    }

    #[test]
    fn bad_values_become_errors_or_are_clamped() {
        let input = format!(
            "{{\"id\":1,\"op\":\"score\",\"pcm16\":\"\"}}\n{{\"id\":2,\"op\":\"score\",\"pcm16\":\"{}\"}}\n\
             {{\"id\":3,\"op\":\"score\",\"pcm16\":\"{}\"}}\n{{\"id\":4,\"op\":\"score\",\"pcm16\":\"@@\"}}\n",
            pcm(1),
            pcm(2)
        );
        let (lines, _) = run(&input);
        assert_eq!(lines.len(), 4);
        assert_eq!(lines[0]["message"], "puste nagranie");
        assert_eq!(lines[1]["type"], "error");
        assert!((lines[2]["probability"].as_f64().unwrap() - 1.0).abs() < 1e-6);
        assert_eq!(lines[3]["type"], "error");
    }

    #[test]
    fn garbage_ping_unknown_and_quit() {
        let input = "śmieci\n\n{\"id\":5,\"op\":\"ping\"}\n{\"id\":6,\"op\":\"nic\"}\n\
                     {\"id\":7,\"op\":\"quit\"}\n{\"id\":8,\"op\":\"ping\"}\n";
        let (lines, _) = run(input);
        assert_eq!(lines.len(), 2);
        assert_eq!(
            (lines[0]["type"].as_str(), lines[0]["id"].as_u64()),
            (Some("done"), Some(5))
        );
        assert_eq!(lines[1]["type"], "error");
    }

    #[test]
    fn pcm16_decodes_full_scale_and_drops_odd_byte() {
        let v = pcm16_to_f32(&[0x00, 0x80, 0xFF, 0x7F, 0x01]);
        assert_eq!(v.len(), 2);
        assert!((v[0] + 1.0).abs() < 1e-6);
        assert!((v[1] - 0.99997).abs() < 1e-4);
    }
}
