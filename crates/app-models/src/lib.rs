//! Menedżer modeli i silników w aplikacji (`app-*`, crates/README.md; SPEC: `docs/modules/models/SPEC.md`):
//! - [`catalog`] — pozycje (LLM z `providers-local`, embeddery z `lib_embed::CATALOG`, whisper, Piper,
//!   Silero VAD, cechy openWakeWord, model mówcy, sidecary) z rozmiarem, licencją, źródłem,
//!   SHA-256 (gdzie przypięty) i miejscem docelowym; pozycje niepotwierdzone — „do potwierdzenia”;
//! - [`fetch`] — HTTPS z `Range` i wznawianiem, SHA-256 w locie, limit rozmiaru, anulowanie;
//! - [`unpack`] — bezpieczne rozpakowanie ZIP (path traversal, dowiązania, duplikaty, zip-bomb);
//! - zadania: limit równoległości, postęp `{file, done, total}`, zgoda TOFU z policzonym hashem
//!   (pozycje bez przypiętego hasha), weryfikacja, usuwanie;
//! - [`embed`] — embedder wyszukiwania (`[search.embedder] model`) i przebudowa wektorów.
//!
//! Bez sekretów w plikach i bez telemetrii (stały `User-Agent`, bez ciasteczek).

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

pub mod catalog;
pub mod data;
pub mod embed;
pub mod fetch;
pub mod install;
mod jobs;
pub mod store;
pub mod unpack;
mod view;

use std::collections::HashMap;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use app_api::dto::{
    AlfaEvent, EmbedderView, ModelItem, ModelItemKind, ModelItemState, ModelsView, ReindexView,
    TrustedHashes,
};
use app_api::{AppError, AppPaths, EventHub};
use search_impl::ReindexOptions;
use tokio::sync::Semaphore;

pub use catalog::{ItemSpec, builtin};
pub use embed::{EMBEDDER_KEY, EmbedDeps, Embedding, LEXICAL, startup_embedder};
pub use view::Live;

use crate::install::Verdict;
use crate::jobs::{JobHandle, Work};
use crate::store::Store;

/// Ustawienia menedżera.
#[derive(Debug, Clone)]
pub struct ModelsOptions {
    /// Limit równoległych pobrań.
    pub parallel: usize,
    /// Dopuszcza `http://127.0.0.1` (tylko testy z lokalnym serwerem).
    pub loopback_http: bool,
    /// Kroki przebudowy wektorów.
    pub reindex: ReindexOptions,
    /// Opóźnienie wyboru embeddera i przebudowy po starcie (`None` — bez tego kroku).
    pub startup_delay: Option<Duration>,
}

impl Default for ModelsOptions {
    fn default() -> Self {
        Self {
            parallel: 2,
            loopback_http: false,
            reindex: ReindexOptions::default(),
            startup_delay: Some(Duration::from_secs(20)),
        }
    }
}

/// Zależności złożenia.
pub struct ModelsDeps {
    /// Katalogi aplikacji.
    pub paths: AppPaths,
    /// Katalog pozycji ([`builtin`] w produkcji).
    pub catalog: Vec<ItemSpec>,
    /// Zdarzenia UI.
    pub events: Option<EventHub>,
    /// Embedder wyszukiwania (`None` — bez wyszukiwania semantycznego).
    pub embed: Option<EmbedDeps>,
    /// Ustawienia.
    pub options: ModelsOptions,
}

/// Menedżer modeli i silników.
pub struct ModelsApp {
    store: Store,
    catalog: Vec<ItemSpec>,
    fetcher: Option<fetch::Fetcher>,
    events: Option<EventHub>,
    permits: Arc<Semaphore>,
    parallel: usize,
    jobs: Mutex<HashMap<String, JobHandle>>,
    live: Mutex<HashMap<String, Live>>,
    embedding: Option<Arc<Embedding>>,
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

impl ModelsApp {
    /// Składa menedżer; po `startup_delay` wybiera embedder z ustawień i zaczyna przebudowę.
    pub fn open(d: ModelsDeps) -> Arc<Self> {
        let fetcher = fetch::Fetcher::new(d.options.loopback_http)
            .inspect_err(
                |e| tracing::warn!(error = %e, "klient HTTPS menedżera modeli niedostępny"),
            )
            .ok();
        let parallel = d.options.parallel.max(1);
        let models = d.paths.models();
        let embedding = d.embed.map(|deps| {
            Arc::new(Embedding::new(
                deps,
                models,
                d.paths.state().join("models").join("reindex.json"),
                d.events.clone(),
                d.options.reindex,
            ))
        });
        let catalog = d.catalog.into_iter().filter(ItemSpec::is_safe).collect();
        let app = Arc::new(Self {
            store: Store::new(d.paths),
            catalog,
            fetcher,
            events: d.events,
            permits: Arc::new(Semaphore::new(parallel)),
            parallel,
            jobs: Mutex::new(HashMap::new()),
            live: Mutex::new(HashMap::new()),
            embedding,
        });
        if let (Some(e), Some(delay)) = (app.embedding.clone(), d.options.startup_delay) {
            tokio::spawn(async move {
                tokio::time::sleep(delay).await;
                e.startup().await;
            });
        }
        app
    }

    fn spec(&self, id: &str) -> Result<&ItemSpec, AppError> {
        self.catalog
            .iter()
            .find(|s| s.id == id)
            .ok_or_else(|| AppError::not_found(format!("pozycja katalogu „{id}”")))
    }

    fn set_live(&self, id: &str, f: impl FnOnce(&mut Live)) {
        f(lock(&self.live).entry(id.to_owned()).or_default());
    }

    fn busy(&self, id: &str) -> bool {
        lock(&self.jobs).contains_key(id)
    }

    fn item(&self, spec: &ItemSpec) -> ModelItem {
        let live = lock(&self.live).get(&spec.id).cloned().unwrap_or_default();
        let active = spec.kind == ModelItemKind::Embed
            && self
                .embedding
                .as_ref()
                .is_some_and(|e| e.active() == spec.id);
        view::item(spec, &self.store, &live, active)
    }

    fn emit_item(&self, spec: &ItemSpec) {
        if let Some(events) = &self.events {
            events.emit(AlfaEvent::ModelChanged {
                item: Box::new(self.item(spec)),
            });
        }
    }

    fn embedding(&self) -> Result<&Arc<Embedding>, AppError> {
        self.embedding
            .as_ref()
            .ok_or_else(|| AppError::unavailable("Wyszukiwanie semantyczne", "search"))
    }

    /// Model embeddera wybrany w ustawieniach, a nieaktywny, bo nie był zainstalowany → włącz.
    async fn after_install(&self, spec: &ItemSpec) {
        let Some(e) = &self.embedding else { return };
        if spec.kind == ModelItemKind::Embed
            && e.view().configured == spec.id
            && e.active() != spec.id
            && let Err(err) = e.activate(&spec.id, false).await
        {
            tracing::warn!(error = %err, "nowy embedder nie załadował się");
        }
    }

    /// `models_list`.
    pub async fn list(&self) -> Result<ModelsView, AppError> {
        let items = self.catalog.iter().map(|s| self.item(s)).collect();
        let (embedder, reindex) = match &self.embedding {
            Some(e) => (e.view(), e.reindex_status()),
            None => (
                EmbedderView {
                    configured: LEXICAL.into(),
                    active: LEXICAL.into(),
                    index_id: String::new(),
                    dims: 0,
                    error: Some("wyszukiwanie niedostępne".into()),
                },
                embed::reindex_view(&search_impl::ReindexReport::default(), false, ""),
            ),
        };
        Ok(ModelsView {
            items,
            embedder,
            reindex,
            parallel: u32::try_from(self.parallel).unwrap_or(u32::MAX),
        })
    }

    /// `models_download` — start albo wznowienie w tle (postęp: `ModelProgress`, stan: `ModelChanged`).
    pub async fn download(self: &Arc<Self>, item_id: &str) -> Result<ModelItem, AppError> {
        let spec = self.spec(item_id)?.clone();
        if !spec.downloadable() {
            return Err(AppError::invalid(format!(
                "„{}” instaluje się ręcznie — zobacz opis pozycji.",
                spec.name
            )));
        }
        let current = self.item(&spec);
        let done = matches!(
            current.state,
            ModelItemState::Installed | ModelItemState::NeedsTrust
        );
        if !done {
            // Trwające zadanie tej pozycji → nic nowego (wpis sprawdzany atomowo w `spawn_job`).
            let _ = self.spawn_job(spec.clone(), Work::Download);
        }
        Ok(self.item(&spec))
    }

    /// `models_cancel` — przerywa pobieranie (plik częściowy zostaje do wznowienia).
    pub async fn cancel(&self, item_id: &str) -> Result<ModelItem, AppError> {
        let spec = self.spec(item_id)?.clone();
        if self.item(&spec).state == ModelItemState::Installing {
            return Err(AppError::invalid(
                "Instalacja w toku — nie można jej przerwać.",
            ));
        }
        self.stop_job(item_id).await;
        Ok(self.item(&spec))
    }

    /// `models_trust_hash` — jawna zgoda na pliki bez przypiętego hasha (hash z karty TOFU).
    pub async fn trust_hash(
        self: &Arc<Self>,
        item_id: &str,
        hashes: TrustedHashes,
    ) -> Result<ModelItem, AppError> {
        let spec = self.spec(item_id)?.clone();
        let pending = self
            .store
            .pending(&spec.id)
            .filter(|_| !self.busy(&spec.id))
            .ok_or_else(|| AppError::invalid("Ta pozycja nie czeka na zgodę."))?;
        for f in spec.files.iter().filter(|f| f.sha256.is_none()) {
            let shown = hashes.get(&f.name);
            let computed = pending.hashes.get(&f.name);
            if shown.is_none()
                || !shown
                    .zip(computed)
                    .is_some_and(|(a, b)| a.eq_ignore_ascii_case(b))
            {
                return Err(AppError::invalid(format!(
                    "SHA-256 pliku {} różni się od policzonego przy pobraniu — zgoda odrzucona.",
                    f.name
                )));
            }
        }
        if !self.spawn_job(spec.clone(), Work::Install(pending.hashes)) {
            return Err(AppError::invalid("Ta pozycja jest właśnie instalowana."));
        }
        Ok(self.item(&spec))
    }

    /// `models_verify` — ponowne SHA-256 zainstalowanych plików.
    pub async fn verify(&self, item_id: &str) -> Result<ModelItem, AppError> {
        let spec = self.spec(item_id)?.clone();
        if self.busy(&spec.id) {
            return Err(AppError::invalid(
                "Pozycja jest właśnie pobierana albo instalowana.",
            ));
        }
        let receipt = self.store.receipt(&spec.id);
        let target = spec.target(self.store.paths());
        let (owned, r) = (spec.clone(), receipt.clone());
        let verdict =
            tokio::task::spawn_blocking(move || install::verify(&owned, &target, r.as_ref()))
                .await
                .map_err(AppError::internal)?;
        let corrupt = match &verdict {
            Verdict::Ok(_) | Verdict::Unknown(_) => None,
            Verdict::Corrupt(why) => Some(why.clone()),
        };
        match receipt {
            Some(mut r) => {
                r.corrupt = corrupt;
                self.store.save_receipt(&r).map_err(AppError::storage)?;
                self.set_live(&spec.id, |l| l.error = None);
            }
            None => self.set_live(&spec.id, |l| {
                l.error = corrupt.map(|c| format!("Weryfikacja: {c}"))
            }),
        }
        let mut item = self.item(&spec);
        if let Verdict::Unknown(computed) | Verdict::Ok(computed) = verdict {
            for f in &mut item.files {
                f.sha256 = f.sha256.take().or_else(|| computed.get(&f.name).cloned());
            }
        }
        self.emit_item(&spec);
        Ok(item)
    }

    /// `models_remove` — usuwa pliki, częściowe pobrania i rekord (nie aktywnego embeddera).
    pub async fn remove(&self, item_id: &str) -> Result<ModelItem, AppError> {
        let spec = self.spec(item_id)?.clone();
        if spec.kind == ModelItemKind::Embed
            && self
                .embedding
                .as_ref()
                .is_some_and(|e| e.active() == spec.id)
        {
            return Err(AppError::forbidden(
                "To aktywny model wyszukiwania — najpierw przełącz wyszukiwanie na inny.",
            ));
        }
        if self.item(&spec).state == ModelItemState::Installing {
            return Err(AppError::invalid("Instalacja w toku — spróbuj za chwilę."));
        }
        self.stop_job(item_id).await;
        let (store, owned) = (self.store.clone(), spec.clone());
        tokio::task::spawn_blocking(move || install::remove(&store, &owned))
            .await
            .map_err(AppError::internal)?
            .map_err(AppError::storage)?;
        self.set_live(&spec.id, |l| *l = Live::default());
        self.emit_item(&spec);
        Ok(self.item(&spec))
    }

    /// `embed_model_activate` — `lexical` albo zainstalowany model embeddera.
    pub async fn activate_embedder(&self, model: &str) -> Result<EmbedderView, AppError> {
        let e = self.embedding()?;
        if model != LEXICAL {
            let spec = self.spec(model)?;
            if spec.kind != ModelItemKind::Embed {
                return Err(AppError::invalid(format!(
                    "„{model}” nie jest modelem embeddingów."
                )));
            }
        }
        let view = e.activate(model, true).await?;
        for spec in self
            .catalog
            .iter()
            .filter(|s| s.kind == ModelItemKind::Embed)
        {
            self.emit_item(spec);
        }
        Ok(view)
    }

    /// `search_reindex_start` — przebudowa wektorów teraz (wznawia od kursora w bazie).
    pub async fn reindex_start(&self) -> Result<ReindexView, AppError> {
        self.embedding()?.restart_reindex().await
    }

    /// `search_reindex_cancel`.
    pub async fn reindex_cancel(&self) -> Result<ReindexView, AppError> {
        Ok(self.embedding()?.cancel_reindex())
    }

    /// `search_reindex_status`.
    pub async fn reindex_status(&self) -> Result<ReindexView, AppError> {
        Ok(self.embedding()?.reindex_status())
    }
}
