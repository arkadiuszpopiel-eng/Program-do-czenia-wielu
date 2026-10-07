//! [`OnnxEmbedder`] — `search_contract::Embedder` na modelu ONNX w wątku tła. Identyfikator i wymiar
//! z manifestu (bez ładowania modelu); dokumenty z `passage_prefix`, zapytania z `query_prefix`;
//! wywołanie dzielone na zlecenia po ≤ [`MAX_TEXTS_PER_JOB`] tekstów (zapytanie nie czeka za całą
//! reindeksacją); tekst przycinany do `max_tokens × 64` bajtów przed tokenizacją.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::mpsc::{self, Sender};
use std::thread::JoinHandle;
use std::time::Duration;

use model_residency_contract::Residency;
use search_contract::{Embedder, SearchError};

use crate::error::EmbedError;
use crate::manifest::EmbedManifest;
use crate::residency::{RESIDENCY_OWNER, UnloadOnRevoke};
use crate::worker::{Job, Stats, StatsSnapshot, WorkerConfig, run};

/// Maksymalna liczba tekstów w jednym zleceniu dla wątku.
pub const MAX_TEXTS_PER_JOB: usize = 64;

/// Bajtów tekstu na token limitu (przycięcie bardzo długich tekstów przed tokenizacją).
const BYTES_PER_TOKEN: usize = 64;

/// Konfiguracja przed uruchomieniem wątku.
pub struct OnnxEmbedderBuilder {
    manifest: EmbedManifest,
    dir: PathBuf,
    residency: Option<Arc<dyn Residency>>,
    idle: Option<Duration>,
    retry_after: Duration,
}

impl OnnxEmbedderBuilder {
    /// Dzierżawa RAM w `model-residency` (słuchacz właściciela [`RESIDENCY_OWNER`]).
    #[must_use]
    pub fn residency(mut self, residency: Arc<dyn Residency>) -> Self {
        self.residency = Some(residency);
        self
    }

    /// Zwolnienie po bezczynności (domyślnie z manifestu; `None` = nigdy).
    #[must_use]
    pub fn idle_unload(mut self, idle: Option<Duration>) -> Self {
        self.idle = idle;
        self
    }

    /// Przerwa po nieudanym ładowaniu (domyślnie 30 s).
    #[must_use]
    pub fn retry_after(mut self, retry: Duration) -> Self {
        self.retry_after = retry;
        self
    }

    /// Uruchamia wątek tła (model ładowany przy pierwszym użyciu albo [`OnnxEmbedder::preload`]).
    pub fn spawn(self) -> Result<OnnxEmbedder, EmbedError> {
        let (tx, rx) = mpsc::channel::<Job>();
        let stats = Arc::new(Stats::default());
        if let Some(r) = &self.residency {
            r.listen(RESIDENCY_OWNER, Arc::new(UnloadOnRevoke::new(tx.clone())));
        }
        let m = &self.manifest;
        let mut embedder = OnnxEmbedder {
            model_id: m.model_id(),
            dims: m.dims,
            query_prefix: m.query_prefix.clone(),
            passage_prefix: m.passage_prefix.clone(),
            max_bytes: m.max_tokens.saturating_mul(BYTES_PER_TOKEN),
            jobs: tx,
            stats: stats.clone(),
            thread: None,
        };
        let cfg = WorkerConfig {
            manifest: self.manifest,
            dir: self.dir,
            residency: self.residency,
            idle: self.idle,
            retry_after: self.retry_after,
        };
        let thread = std::thread::Builder::new()
            .name("alfa-embedder".into())
            .spawn(move || run(cfg, &rx, stats))
            .map_err(|e| EmbedError::Model(format!("wątek embeddera: {e}")))?;
        embedder.thread = Some(thread);
        Ok(embedder)
    }
}

/// Embedder ONNX.
pub struct OnnxEmbedder {
    model_id: String,
    dims: usize,
    query_prefix: String,
    passage_prefix: String,
    max_bytes: usize,
    jobs: Sender<Job>,
    stats: Arc<Stats>,
    thread: Option<JoinHandle<()>>,
}

impl std::fmt::Debug for OnnxEmbedder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OnnxEmbedder")
            .field("model_id", &self.model_id)
            .field("dims", &self.dims)
            .finish_non_exhaustive()
    }
}

/// Przycina tekst do `max` bajtów na granicy znaku.
fn clip(text: &str, max: usize) -> &str {
    if text.len() <= max {
        return text;
    }
    let mut end = max;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    &text[..end]
}

impl OnnxEmbedder {
    /// Konfiguracja z manifestu w pamięci (`dir` = katalog plików modelu).
    pub fn builder(manifest: EmbedManifest, dir: impl Into<PathBuf>) -> OnnxEmbedderBuilder {
        let idle =
            (manifest.idle_unload_s > 0).then(|| Duration::from_secs(manifest.idle_unload_s));
        OnnxEmbedderBuilder {
            manifest,
            dir: dir.into(),
            residency: None,
            idle,
            retry_after: Duration::from_secs(30),
        }
    }

    /// Konfiguracja z pliku manifestu (`embed.json`); pliki sprawdzane przy ładowaniu.
    pub fn from_manifest_file(path: &Path) -> Result<OnnxEmbedderBuilder, EmbedError> {
        let (manifest, dir) = EmbedManifest::load(path)?;
        Ok(Self::builder(manifest, dir))
    }

    fn run_jobs(&self, prefix: &str, texts: &[&str]) -> Result<Vec<Vec<f32>>, EmbedError> {
        let mut out = Vec::with_capacity(texts.len());
        for chunk in texts.chunks(MAX_TEXTS_PER_JOB) {
            let prepared = chunk
                .iter()
                .map(|t| format!("{prefix}{}", clip(t, self.max_bytes)))
                .collect();
            let (reply, rx) = mpsc::sync_channel(1);
            self.jobs
                .send(Job::Embed {
                    texts: prepared,
                    reply,
                })
                .map_err(|_| EmbedError::Stopped)?;
            out.extend(rx.recv().map_err(|_| EmbedError::Stopped)??);
        }
        Ok(out)
    }

    /// Wektory dokumentów (`passage_prefix`).
    pub fn embed_passages(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>, EmbedError> {
        self.run_jobs(&self.passage_prefix, texts)
    }

    /// Wektory zapytań (`query_prefix`).
    pub fn embed_queries(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>, EmbedError> {
        self.run_jobs(&self.query_prefix, texts)
    }

    /// Ładuje model teraz (np. po przełączeniu w UI — błąd od razu, nie przy pierwszym zapytaniu).
    pub fn preload(&self) -> Result<(), EmbedError> {
        let (reply, rx) = mpsc::sync_channel(1);
        self.jobs
            .send(Job::Preload(reply))
            .map_err(|_| EmbedError::Stopped)?;
        rx.recv().map_err(|_| EmbedError::Stopped)?
    }

    /// Zleca wyładowanie modelu (zwolnienie RAM i dzierżawy).
    pub fn unload(&self) {
        let _ = self.jobs.send(Job::Unload);
    }

    /// Liczniki wątku.
    pub fn stats(&self) -> StatsSnapshot {
        self.stats.snapshot()
    }
}

impl Drop for OnnxEmbedder {
    fn drop(&mut self) {
        let _ = self.jobs.send(Job::Stop);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

impl Embedder for OnnxEmbedder {
    fn model_id(&self) -> &str {
        &self.model_id
    }

    fn dims(&self) -> usize {
        self.dims
    }

    fn embed(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>, SearchError> {
        Ok(self.embed_passages(texts)?)
    }

    fn embed_query(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>, SearchError> {
        Ok(self.embed_queries(texts)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clip_respects_char_boundaries() {
        assert_eq!(clip("żółw", 3), "ż");
        assert_eq!(clip("żółw", 100), "żółw");
        assert_eq!(clip("ab", 0), "");
    }
}
