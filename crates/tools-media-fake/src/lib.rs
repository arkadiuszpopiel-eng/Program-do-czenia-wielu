//! Atrapy `tools-media` (docs/modules/tools-media/SPEC.md, sekcja „Fake”):
//! - [`FakeTools`] — prawdziwe manifesty i walidacja z kontraktu, wyniki skryptowane (FIFO);
//! - [`FakeTranscoder`] — deterministyczny konwerter: poprawny nagłówek formatu docelowego
//!   (`lib-media::samples`), zapis zadań, „brak ffmpeg”, błąd albo za duży wynik na żądanie;
//! - [`FakePlayer`] — odtwarzacz zapisujący klipy (bez dźwięku), kolejka na żądanie.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

use async_trait::async_trait;
use lib_media::samples;
use tools_common_contract::{
    RecordedCall, Tool, ToolCtx, ToolErrorKind, ToolManifest, ToolOutcome, Toolset,
};
use tools_media_contract::{
    AudioClip, AudioPlayer, CancellationToken, ConvertError, ConvertJob, PlayError, PlayTicket,
    TargetFormat, Transcoder, check_args, manifests,
};

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

#[derive(Debug, Default)]
struct State {
    queue: VecDeque<(String, ToolOutcome)>,
    calls: Vec<(String, RecordedCall)>,
}

/// Atrapa zestawu narzędzi.
#[derive(Debug, Clone, Default)]
pub struct FakeTools {
    state: Arc<Mutex<State>>,
}

impl FakeTools {
    /// Wynik najbliższego wywołania narzędzia `tool` (FIFO).
    pub fn push(&self, tool: &str, outcome: ToolOutcome) {
        lock(&self.state)
            .queue
            .push_back((tool.to_owned(), outcome));
    }

    /// Wywołania (narzędzie, zapis).
    pub fn calls(&self) -> Vec<(String, RecordedCall)> {
        lock(&self.state).calls.clone()
    }
}

struct FakeTool {
    owner: FakeTools,
    manifest: ToolManifest,
}

#[async_trait]
impl Tool for FakeTool {
    fn manifest(&self) -> &ToolManifest {
        &self.manifest
    }

    async fn call(&self, args: serde_json::Value, ctx: &ToolCtx) -> ToolOutcome {
        let name = self.manifest.name.clone();
        if !args.is_object() {
            return ToolOutcome::failed(
                ToolErrorKind::InvalidArgs,
                "Niepoprawne argumenty: oczekiwano obiektu JSON.",
            );
        }
        if let Err(e) = check_args(&name, &args) {
            return ToolOutcome::failed(
                ToolErrorKind::InvalidArgs,
                format!("Niepoprawne argumenty: {e}."),
            );
        }
        if ctx.cancel.is_cancelled() {
            return ToolOutcome::cancelled(&self.manifest.title);
        }
        let mut s = lock(&self.owner.state);
        s.calls.push((
            name.clone(),
            RecordedCall {
                args,
                holder: ctx.holder.clone(),
                step: ctx.step,
                untrusted_args: ctx.untrusted_args,
            },
        ));
        let scripted = s
            .queue
            .iter()
            .position(|(t, _)| *t == name)
            .and_then(|i| s.queue.remove(i))
            .map(|(_, o)| o);
        scripted.unwrap_or_else(|| {
            ToolOutcome::ok(
                format!("{}: wykonano (atrapa).", self.manifest.title),
                serde_json::json!({}),
            )
        })
    }
}

impl Toolset for FakeTools {
    fn tools(&self) -> Vec<Arc<dyn Tool>> {
        manifests()
            .into_iter()
            .map(|manifest| {
                Arc::new(FakeTool {
                    owner: self.clone(),
                    manifest,
                }) as Arc<dyn Tool>
            })
            .collect()
    }
}

/// Minimalny poprawny plik formatu docelowego (nagłówek z `lib-media::samples`).
pub fn sample_output(target: TargetFormat) -> Vec<u8> {
    match target {
        TargetFormat::Wav => samples::wav(48_000, 2, 16, 500),
        TargetFormat::Mp3 => samples::mp3(20),
        TargetFormat::Flac => samples::flac(48_000, 2, 24_000),
        TargetFormat::Ogg | TargetFormat::Opus => samples::ogg_opus(500),
        TargetFormat::M4a | TargetFormat::Mp4 | TargetFormat::Webm => {
            samples::mp4(320, 240, 500, false)
        }
        TargetFormat::Gif => samples::gif(32, 32, 2, 10),
        TargetFormat::Png => samples::png(32, 32, 0),
        TargetFormat::Jpg => samples::jpeg(32, 32),
    }
}

#[derive(Default)]
struct TranscodeState {
    jobs: Vec<ConvertJob>,
    scripted: VecDeque<Result<Vec<u8>, ConvertError>>,
}

/// Deterministyczny konwerter.
#[derive(Clone, Default)]
pub struct FakeTranscoder {
    state: Arc<Mutex<TranscodeState>>,
    missing: Arc<AtomicBool>,
}

impl FakeTranscoder {
    /// Konwerter „zainstalowany”.
    pub fn new() -> Self {
        Self::default()
    }

    /// Udaje brak ffmpeg.
    pub fn set_missing(&self, missing: bool) {
        self.missing.store(missing, Ordering::SeqCst);
    }

    /// Wynik najbliższej konwersji (FIFO), przed domyślnym nagłówkiem.
    pub fn push(&self, result: Result<Vec<u8>, ConvertError>) {
        lock(&self.state).scripted.push_back(result);
    }

    /// Zadania w kolejności.
    pub fn jobs(&self) -> Vec<ConvertJob> {
        lock(&self.state).jobs.clone()
    }
}

impl Transcoder for FakeTranscoder {
    fn available(&self) -> Result<(), ConvertError> {
        if self.missing.load(Ordering::SeqCst) {
            return Err(ConvertError::NotInstalled("atrapa bez ffmpeg".into()));
        }
        Ok(())
    }

    fn convert(&self, job: &ConvertJob, cancel: Arc<AtomicBool>) -> Result<Vec<u8>, ConvertError> {
        self.available()?;
        if cancel.load(Ordering::SeqCst) {
            return Err(ConvertError::Cancelled);
        }
        let mut s = lock(&self.state);
        s.jobs.push(job.clone());
        let bytes = match s.scripted.pop_front() {
            Some(r) => r?,
            None => sample_output(job.target),
        };
        if bytes.len() as u64 > job.max_output_bytes {
            return Err(ConvertError::TooLarge(job.max_output_bytes));
        }
        Ok(bytes)
    }
}

/// Odtwarzacz zapisujący klipy.
#[derive(Clone, Default)]
pub struct FakePlayer {
    clips: Arc<Mutex<Vec<AudioClip>>>,
    busy: Arc<AtomicBool>,
    unavailable: Arc<AtomicBool>,
    next: Arc<AtomicU64>,
    stopped: Arc<AtomicU64>,
}

impl FakePlayer {
    /// Głośnik zajęty (klip „czeka w kolejce”).
    pub fn set_busy(&self, busy: bool) {
        self.busy.store(busy, Ordering::SeqCst);
    }

    /// Głośnik niedostępny.
    pub fn set_unavailable(&self, unavailable: bool) {
        self.unavailable.store(unavailable, Ordering::SeqCst);
    }

    /// Przyjęte klipy.
    pub fn clips(&self) -> Vec<AudioClip> {
        lock(&self.clips).clone()
    }

    /// Ile razy zatrzymano wszystko.
    pub fn stops(&self) -> u64 {
        self.stopped.load(Ordering::SeqCst)
    }
}

#[async_trait]
impl AudioPlayer for FakePlayer {
    async fn play(
        &self,
        clip: AudioClip,
        cancel: CancellationToken,
    ) -> Result<PlayTicket, PlayError> {
        if self.unavailable.load(Ordering::SeqCst) {
            return Err(PlayError::Unavailable("atrapa bez głośnika".into()));
        }
        if clip.samples.is_empty() || !(1..=2).contains(&clip.channels) {
            return Err(PlayError::Format(
                "pusty klip albo zła liczba kanałów".into(),
            ));
        }
        let ticket = PlayTicket {
            id: self.next.fetch_add(1, Ordering::SeqCst) + 1,
            duration_ms: clip.duration_ms(),
            queued: self.busy.load(Ordering::SeqCst),
        };
        if !cancel.is_cancelled() {
            lock(&self.clips).push(clip);
        }
        Ok(ticket)
    }

    fn stop_all(&self) -> usize {
        self.stopped.fetch_add(1, Ordering::SeqCst);
        lock(&self.clips).len()
    }
}
