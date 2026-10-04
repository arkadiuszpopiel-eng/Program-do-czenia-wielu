//! Embedder wyszukiwania w aplikacji (F7-02, `crates/lib-embed/README.md` pkt 2–4, 6):
//! `[search.embedder] model = "multilingual-e5-small" | "lexical"` — `OnnxEmbedder` z dzierżawą
//! `model-residency` (ładowanie `preload` w tle), gdy model jest zainstalowany, inaczej embedder
//! leksykalny kompozycji. Wymiana embeddera w działającym `SqliteSearch` (`set_embedder`) →
//! przebudowa wektorów w tle (`spawn_reindex`) dla baz sesji i baz zakresów pamięci; uchwyt
//! w stanie aplikacji, postęp jako zdarzenia `ReindexStatus` (tylko liczniki).

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

use app_api::dto::{AlfaEvent, EmbedderView, ReindexView};
use app_api::{AppError, EventHub};
use core_config_contract::{ConfigKey, ConfigLayer, ConfigStore, Origin, Scope};
use lib_embed::OnnxEmbedder;
use lib_sqlstore::Db;
use memory_contract::MemoryScope;
use memory_impl::ScopeDbs;
use model_residency_contract::Residency;
use search_contract::{Embedder, SearchError, SessionId};
use search_impl::{ReindexHandle, ReindexOptions, ReindexReport, ReindexSource, SqliteSearch};

/// Klucz ustawienia wyboru embeddera.
pub const EMBEDDER_KEY: &str = "search.embedder.model";
/// Wybór „bez modelu” — embedder leksykalny kompozycji.
pub const LEXICAL: &str = "lexical";
const PROGRESS_EVERY: Duration = Duration::from_millis(250);

/// Katalog modelu embeddera `id` w katalogu modeli.
pub fn model_dir(models: &Path, id: &str) -> PathBuf {
    models.join("embed").join(id)
}

/// Wybór z ustawień (brak = model domyślny `lib-embed`, używany, gdy jest zainstalowany).
pub fn configured(value: Option<&serde_json::Value>) -> String {
    value
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .unwrap_or(lib_embed::DEFAULT_MODEL)
        .to_owned()
}

/// Embedder na start `search` (przed złożeniem rezydencji): zainstalowany model z ustawień
/// (ładowany leniwie — pierwsze użycie albo `preload` po starcie) albo embedder leksykalny.
pub fn startup_embedder(
    models: &Path,
    setting: Option<&serde_json::Value>,
    lexical: Arc<dyn Embedder>,
) -> Arc<dyn Embedder> {
    let id = configured(setting);
    if id == LEXICAL {
        return lexical;
    }
    let Some(manifest) = lib_embed::installed(&model_dir(models, &id)) else {
        return lexical;
    };
    match OnnxEmbedder::from_manifest_file(&manifest).and_then(|b| b.spawn()) {
        Ok(e) => Arc::new(e),
        Err(e) => {
            tracing::warn!(model = %id, error = %e, "embedder niedostępny — wyszukiwanie leksykalne");
            lexical
        }
    }
}

/// Bazy zakresów pamięci (projekt/agentka/globalna) jako źródło przebudowy; bazy sesji przegląda
/// sam `search`.
pub struct ScopeSource(pub Arc<dyn ScopeDbs>);

impl ReindexSource for ScopeSource {
    fn databases(&self) -> Result<Vec<(SessionId, Arc<Db>)>, SearchError> {
        let mut out = Vec::new();
        for scope in self.0.known().map_err(SearchError::storage)? {
            if matches!(scope, MemoryScope::Session(_)) {
                continue;
            }
            if let Some(db) = self.0.db(&scope, false).map_err(SearchError::storage)? {
                out.push((memory_impl::index_label(&scope), db));
            }
        }
        Ok(out)
    }
}

/// Zależności embeddera aplikacji.
pub struct EmbedDeps {
    /// Wyszukiwanie (ten sam obiekt indeksuje sesje i pamięć).
    pub search: Arc<SqliteSearch>,
    /// Embedder leksykalny kompozycji (wybór `lexical` i zapas).
    pub lexical: Arc<dyn Embedder>,
    /// Zarządca RAM (dzierżawa modelu).
    pub residency: Option<Arc<dyn Residency>>,
    /// Bazy zakresów pamięci.
    pub scopes: Option<Arc<dyn ScopeDbs>>,
    /// Ustawienia.
    pub config: Arc<dyn ConfigStore>,
}

#[derive(Debug, Clone, Default)]
struct Current {
    configured: String,
    active: String,
    error: Option<String>,
}

/// Embedder aplikacji i przebudowa wektorów.
pub struct Embedding {
    deps: EmbedDeps,
    models: PathBuf,
    marker: PathBuf,
    events: Option<EventHub>,
    opts: ReindexOptions,
    current: Mutex<Current>,
    reindex: Mutex<Option<ReindexHandle>>,
    switching: tokio::sync::Mutex<()>,
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Widok przebudowy z raportu.
pub fn reindex_view(report: &ReindexReport, running: bool, embedder: &str) -> ReindexView {
    ReindexView {
        running,
        embedder: embedder.to_owned(),
        databases: report.databases as u64,
        rebuilt: report.rebuilt as u64,
        embedded: report.embedded,
        done: report.current.done,
        total: report.current.total,
        failed: report.failed.len() as u64,
        cancelled: report.cancelled,
        finished: report.finished,
    }
}

/// Pełny przebieg przebudowy na starcie co najwyżej raz na tydzień dla tego samego embeddera.
pub const FULL_PASS_EVERY: Duration = Duration::from_secs(7 * 24 * 3600);

#[derive(serde::Serialize, serde::Deserialize)]
struct PassMarker {
    embedder: String,
    finished_unix_s: u64,
}

fn now_unix_s() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

/// Zapis: pełny przebieg bez błędów dla embeddera `embedder` (bez treści — tylko identyfikator).
fn record_pass(marker: &Path, embedder: &str) {
    let pass = PassMarker {
        embedder: embedder.to_owned(),
        finished_unix_s: now_unix_s(),
    };
    let written = marker
        .parent()
        .map_or(Ok(()), std::fs::create_dir_all)
        .and_then(|()| {
            let text = serde_json::to_string(&pass).map_err(std::io::Error::other)?;
            std::fs::write(marker, text)
        });
    if let Err(e) = written {
        tracing::warn!(error = %e, "zapis stanu przebudowy wektorów nie powiódł się");
    }
}

/// Czy ostatni pełny przebieg był dla `embedder` i niedawno.
pub fn fresh_pass(marker: &Path, embedder: &str) -> bool {
    let Ok(text) = std::fs::read_to_string(marker) else {
        return false;
    };
    serde_json::from_str::<PassMarker>(&text).is_ok_and(|p| {
        p.embedder == embedder
            && now_unix_s().saturating_sub(p.finished_unix_s) < FULL_PASS_EVERY.as_secs()
    })
}

fn index_id(e: &dyn Embedder) -> String {
    format!("{}/{}", e.model_id(), e.dims())
}

impl Embedding {
    /// Embedder aplikacji nad `search`.
    pub fn new(
        deps: EmbedDeps,
        models: PathBuf,
        marker: PathBuf,
        events: Option<EventHub>,
        opts: ReindexOptions,
    ) -> Self {
        // Do kroku startowego: embedder, z którym zbudowano `search` (`startup_embedder`).
        let id = deps.search.embedder().model_id().to_owned();
        let active = if id == deps.lexical.model_id() {
            LEXICAL.to_owned()
        } else {
            id.split('@').next().unwrap_or_default().to_owned()
        };
        let current = Current {
            configured: active.clone(),
            active,
            error: None,
        };
        Self {
            deps,
            models,
            marker,
            events,
            opts,
            current: Mutex::new(current),
            reindex: Mutex::new(None),
            switching: tokio::sync::Mutex::new(()),
        }
    }

    async fn setting(&self) -> String {
        let value = match ConfigKey::new(EMBEDDER_KEY) {
            Ok(key) => self
                .deps
                .config
                .get(&key, &Scope::Global)
                .await
                .ok()
                .flatten(),
            Err(_) => None,
        };
        configured(value.as_ref())
    }

    /// Stan embeddera.
    pub fn view(&self) -> EmbedderView {
        let current = lock(&self.current).clone();
        let e = self.deps.search.embedder();
        EmbedderView {
            configured: current.configured,
            active: current.active,
            index_id: index_id(e.as_ref()),
            dims: u32::try_from(e.dims()).unwrap_or(u32::MAX),
            error: current.error,
        }
    }

    /// Wybór w ustawieniach i model w użyciu.
    pub fn active(&self) -> String {
        lock(&self.current).active.clone()
    }

    async fn build(&self, id: &str) -> Result<Arc<dyn Embedder>, AppError> {
        if id == LEXICAL {
            return Ok(self.deps.lexical.clone());
        }
        let manifest = lib_embed::installed(&model_dir(&self.models, id)).ok_or_else(|| {
            AppError::invalid(format!(
                "Model „{id}” nie jest zainstalowany — pobierz go w Ustawieniach → Modele i silniki."
            ))
        })?;
        let residency = self.deps.residency.clone();
        let loaded = tokio::task::spawn_blocking(move || {
            let mut builder = OnnxEmbedder::from_manifest_file(&manifest)?;
            if let Some(r) = residency {
                builder = builder.residency(r);
            }
            let embedder = builder.spawn()?;
            embedder.preload()?;
            Ok::<_, lib_embed::EmbedError>(embedder)
        })
        .await
        .map_err(AppError::internal)?;
        let embedder = loaded
            .map_err(|e| AppError::invalid(format!("Model „{id}” nie załadował się: {e}")))?;
        Ok(Arc::new(embedder))
    }

    /// Przełącza embedder (`persist` — zapis wyboru w ustawieniach) i zaczyna przebudowę wektorów.
    pub async fn activate(&self, id: &str, persist: bool) -> Result<EmbedderView, AppError> {
        let _guard = self.switching.lock().await;
        self.switch(id, persist).await?;
        self.restart_reindex().await?;
        Ok(self.view())
    }

    async fn switch(&self, id: &str, persist: bool) -> Result<(), AppError> {
        let embedder = self.build(id).await?;
        if persist {
            let key = ConfigKey::new(EMBEDDER_KEY).map_err(AppError::from)?;
            let value = Some(serde_json::Value::String(id.to_owned()));
            self.deps
                .config
                .set(
                    &key,
                    value,
                    &Scope::Global,
                    &ConfigLayer::Shared,
                    Origin::User,
                )
                .await
                .map_err(AppError::from)?;
        }
        self.deps.search.set_embedder(embedder);
        *lock(&self.current) = Current {
            configured: id.to_owned(),
            active: id.to_owned(),
            error: None,
        };
        Ok(())
    }

    /// Start aplikacji: wybór z ustawień; błąd ładowania → embedder leksykalny z opisem błędu.
    /// Przebudowa (wznawianie przerwanej, uzupełnianie brakujących wektorów) — gdy ostatni pełny
    /// przebieg był dla innego embeddera albo dawniej niż [`FULL_PASS_EVERY`] (otwiera wszystkie bazy).
    pub async fn startup(&self) {
        let _guard = self.switching.lock().await;
        let id = self.setting().await;
        if let Err(e) = self.switch(&id, false).await {
            let installed = lib_embed::installed(&model_dir(&self.models, &id)).is_some();
            self.deps.search.set_embedder(self.deps.lexical.clone());
            *lock(&self.current) = Current {
                configured: id,
                active: LEXICAL.into(),
                // Brak modelu domyślnego to zwykły stan (wyszukiwanie leksykalne), nie błąd.
                error: installed.then(|| e.to_string()),
            };
        }
        let current = index_id(self.deps.search.embedder().as_ref());
        if !fresh_pass(&self.marker, &current)
            && let Err(e) = self.restart_reindex().await
        {
            tracing::warn!(error = %e, "przebudowa wektorów nie wystartowała");
        }
    }

    /// Uruchamia przebudowę od nowa (poprzednia jest anulowana; kursor w bazie — bez utraty pracy).
    pub async fn restart_reindex(&self) -> Result<ReindexView, AppError> {
        let old = lock(&self.reindex).take();
        if let Some(old) = old {
            // `drop` czeka na wątek (koniec bieżącego kroku) — poza wątkiem runtime.
            let _ = tokio::task::spawn_blocking(move || drop(old)).await;
        }
        let sources: Vec<Arc<dyn ReindexSource>> = self
            .deps
            .scopes
            .iter()
            .map(|s| Arc::new(ScopeSource(s.clone())) as Arc<dyn ReindexSource>)
            .collect();
        let embedder = index_id(self.deps.search.embedder().as_ref());
        let events = self.events.clone();
        let last = Mutex::new(None::<Instant>);
        let label = embedder.clone();
        let marker = self.marker.clone();
        let progress = Arc::new(move |r: &ReindexReport| {
            if r.finished && !r.cancelled && r.failed.is_empty() {
                record_pass(&marker, &label);
            }
            let Some(events) = &events else { return };
            let mut last = lock(&last);
            if !r.finished && last.is_some_and(|t| t.elapsed() < PROGRESS_EVERY) {
                return;
            }
            *last = Some(Instant::now());
            events.emit(AlfaEvent::ReindexStatus {
                status: reindex_view(r, !r.finished, &label),
            });
        });
        let handle = self
            .deps
            .search
            .spawn_reindex(sources, self.opts, Some(progress))
            .map_err(AppError::storage)?;
        let view = reindex_view(&handle.snapshot(), true, &embedder);
        *lock(&self.reindex) = Some(handle);
        Ok(view)
    }

    /// Anuluje przebudowę (wznowi się przy następnym starcie albo „Przebuduj teraz”).
    pub fn cancel_reindex(&self) -> ReindexView {
        if let Some(h) = lock(&self.reindex).as_ref() {
            h.cancel();
        }
        self.reindex_status()
    }

    /// Stan przebudowy.
    pub fn reindex_status(&self) -> ReindexView {
        let embedder = index_id(self.deps.search.embedder().as_ref());
        match lock(&self.reindex).as_ref() {
            Some(h) => {
                let r = h.snapshot();
                reindex_view(&r, !r.finished, &embedder)
            }
            None => reindex_view(&ReindexReport::default(), false, &embedder),
        }
    }
}
