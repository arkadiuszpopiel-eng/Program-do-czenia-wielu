//! Klient procesu `alfa-smart-turn`: [`SidecarScorer`] wysyła ostatnie 8 s nagrania (PCM16
//! w base64, linie JSON) i czeka na wynik z limitem czasu. Po błędzie proces startuje od nowa,
//! a po kilku błędach z rzędu dostaje przerwę — rozmowa w tym czasie korzysta z oceny z tekstu.

use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{Receiver, RecvTimeoutError, channel};
use std::sync::{Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

use base64::Engine as _;
use voice_turn_contract::TurnError;

use crate::smart_turn::{AudioScorer, WINDOW_SAMPLES, model_err};

/// Błędy z rzędu, po których proces dostaje przerwę.
const MAX_FAILURES: u32 = 3;
/// Długość przerwy.
const COOLDOWN: Duration = Duration::from_secs(60);

/// Uruchomienie procesu `alfa-smart-turn`.
#[derive(Debug, Clone)]
pub struct SidecarCfg {
    /// Program `alfa-smart-turn`.
    pub program: PathBuf,
    /// Plik modelu.
    pub model: PathBuf,
    /// Biblioteka ONNX Runtime.
    pub runtime: PathBuf,
    /// Limit czasu pierwszej oceny po starcie procesu (wczytanie modelu).
    pub first_timeout: Duration,
    /// Limit czasu kolejnych ocen.
    pub timeout: Duration,
}

impl SidecarCfg {
    /// Ustawienia domyślne dla podanych plików.
    pub fn new(program: PathBuf, model: PathBuf, runtime: PathBuf) -> Self {
        Self {
            program,
            model,
            runtime,
            first_timeout: Duration::from_millis(2500),
            timeout: Duration::from_millis(800),
        }
    }
}

struct Proc {
    child: Child,
    stdin: ChildStdin,
    lines: Receiver<String>,
    fresh: bool,
}

impl Drop for Proc {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[derive(Default)]
struct State {
    proc: Option<Proc>,
    next_id: u64,
    failures: u32,
    paused_until: Option<Instant>,
}

/// Klient procesu `alfa-smart-turn`.
pub struct SidecarScorer {
    cfg: SidecarCfg,
    state: Mutex<State>,
}

impl std::fmt::Debug for SidecarScorer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SidecarScorer")
            .field("cfg", &self.cfg)
            .finish_non_exhaustive()
    }
}

/// PCM16 LE w base64 z ostatnich [`WINDOW_SAMPLES`] próbek.
pub fn encode_tail(samples: &[f32]) -> String {
    let tail = &samples[samples.len().saturating_sub(WINDOW_SAMPLES)..];
    let mut bytes = Vec::with_capacity(tail.len() * 2);
    for s in tail {
        #[allow(clippy::cast_possible_truncation)]
        let v = (s.clamp(-1.0, 1.0) * 32_767.0).round() as i16;
        bytes.extend_from_slice(&v.to_le_bytes());
    }
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

/// Odczyt odpowiedzi o numerze `id`: `Some(Ok(p))`, `Some(Err(_))` albo `None` dla innej linii.
pub fn parse_reply(line: &str, id: u64) -> Option<Result<f32, String>> {
    let value: serde_json::Value = serde_json::from_str(line.trim()).ok()?;
    if value["id"].as_u64() != Some(id) {
        return None;
    }
    match value["type"].as_str()? {
        #[allow(clippy::cast_possible_truncation)]
        "score" => Some(
            value["probability"]
                .as_f64()
                .map(|p| p as f32)
                .ok_or_else(|| "odpowiedź bez wyniku".to_owned()),
        ),
        "error" => Some(Err(value["message"]
            .as_str()
            .unwrap_or("błąd procesu")
            .to_owned())),
        _ => None,
    }
}

impl SidecarScorer {
    /// Klient; proces startuje od razu, żeby model był wczytany przed pierwszą wypowiedzią.
    pub fn new(cfg: SidecarCfg) -> Self {
        let scorer = Self {
            cfg,
            state: Mutex::new(State::default()),
        };
        {
            let mut st = scorer.lock();
            st.proc = scorer.spawn().ok();
        }
        scorer
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn spawn(&self) -> Result<Proc, TurnError> {
        let mut cmd = Command::new(&self.cfg.program);
        cmd.arg("--model")
            .arg(&self.cfg.model)
            .arg("--ort")
            .arg(&self.cfg.runtime)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt as _;
            cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
        }
        let mut child = cmd
            .spawn()
            .map_err(|e| model_err(format!("{}: {e}", self.cfg.program.display())))?;
        lib_sidecar_guard::register(child.id(), &self.cfg.program);
        let (Some(stdin), Some(stdout)) = (child.stdin.take(), child.stdout.take()) else {
            let _ = child.kill();
            return Err(model_err("proces bez potoków"));
        };
        let (tx, lines) = channel();
        std::thread::Builder::new()
            .name("alfa-smart-turn".into())
            .spawn(move || {
                for line in BufReader::new(stdout).lines() {
                    let Ok(line) = line else { break };
                    if tx.send(line).is_err() {
                        break;
                    }
                }
            })
            .map_err(|e| model_err(e.to_string()))?;
        Ok(Proc {
            child,
            stdin,
            lines,
            fresh: true,
        })
    }

    fn ask(&self, st: &mut State, samples: &[f32]) -> Result<f32, TurnError> {
        let alive = st
            .proc
            .as_mut()
            .is_some_and(|p| matches!(p.child.try_wait(), Ok(None)));
        if !alive {
            st.proc = Some(self.spawn()?);
        }
        st.next_id += 1;
        let id = st.next_id;
        let proc = st.proc.as_mut().ok_or_else(|| model_err("brak procesu"))?;
        let line = format!(
            "{{\"id\":{id},\"op\":\"score\",\"pcm16\":\"{}\"}}\n",
            encode_tail(samples)
        );
        proc.stdin
            .write_all(line.as_bytes())
            .and_then(|()| proc.stdin.flush())
            .map_err(|e| model_err(e.to_string()))?;
        let limit = if proc.fresh {
            self.cfg.first_timeout
        } else {
            self.cfg.timeout
        };
        let deadline = Instant::now() + limit;
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            match proc.lines.recv_timeout(left) {
                Ok(reply) => {
                    if let Some(result) = parse_reply(&reply, id) {
                        proc.fresh = false;
                        return result.map_err(model_err);
                    }
                }
                Err(RecvTimeoutError::Timeout) => return Err(model_err("przekroczony czas oceny")),
                Err(RecvTimeoutError::Disconnected) => {
                    return Err(model_err("proces zakończył pracę"));
                }
            }
        }
    }
}

impl AudioScorer for SidecarScorer {
    fn score(&self, samples: &[f32]) -> Result<f32, TurnError> {
        let mut st = self.lock();
        if let Some(until) = st.paused_until {
            if Instant::now() < until {
                return Err(model_err("proces w przerwie po błędach"));
            }
            st.paused_until = None;
            st.failures = 0;
        }
        match self.ask(&mut st, samples) {
            Ok(p) => {
                st.failures = 0;
                Ok(p)
            }
            Err(e) => {
                // Po błędzie proces zaczyna od nowa — zaległa odpowiedź nie pomyli się z następną.
                st.proc = None;
                st.failures += 1;
                if st.failures >= MAX_FAILURES {
                    st.paused_until = Some(Instant::now() + COOLDOWN);
                }
                Err(e)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tail_is_limited_to_the_window_and_clamped() {
        let short = base64::engine::general_purpose::STANDARD
            .decode(encode_tail(&[0.0, 2.0, -2.0]))
            .unwrap();
        assert_eq!(short, vec![0, 0, 0xFF, 0x7F, 0x01, 0x80]);
        let long = base64::engine::general_purpose::STANDARD
            .decode(encode_tail(&vec![0.0; WINDOW_SAMPLES + 1000]))
            .unwrap();
        assert_eq!(long.len(), WINDOW_SAMPLES * 2);
    }

    #[test]
    fn replies_are_matched_by_id() {
        let ok = "{\"type\":\"score\",\"id\":7,\"probability\":0.5,\"ms\":12}";
        assert_eq!(parse_reply(ok, 7), Some(Ok(0.5)));
        assert_eq!(parse_reply(ok, 8), None);
        assert_eq!(parse_reply("śmieci", 7), None);
        assert_eq!(
            parse_reply("{\"type\":\"error\",\"id\":7,\"message\":\"x\"}", 7),
            Some(Err("x".into()))
        );
        assert_eq!(parse_reply("{\"type\":\"done\",\"id\":7}", 7), None);
    }

    #[test]
    fn missing_program_pauses_after_repeated_failures() {
        let cfg = SidecarCfg::new(
            PathBuf::from("nie-ma-takiego-programu-alfa"),
            PathBuf::from("m.onnx"),
            PathBuf::from("ort.dll"),
        );
        let scorer = SidecarScorer::new(cfg);
        for _ in 0..MAX_FAILURES {
            assert!(scorer.score(&[0.0; 4000]).is_err());
        }
        let err = scorer.score(&[0.0; 4000]).unwrap_err().to_string();
        assert!(err.contains("przerwie"), "{err}");
    }
}
