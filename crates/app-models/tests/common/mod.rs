//! Wspólne narzędzia testów `app-models`: lokalny serwer HTTP, pozycje katalogu testowego,
//! menedżer nad katalogiem tymczasowym, czekanie na stan, zbieranie zdarzeń.

#![allow(dead_code, clippy::unwrap_used, clippy::expect_used)]

pub mod http;

use std::sync::{Arc, Mutex};
use std::time::Duration;

use app_api::dto::{AlfaEvent, LocalizedText, ModelItem, ModelItemKind, ModelItemState};
use app_api::{AppPaths, EventHub};
use app_models::catalog::{FileSpec, Install, ItemSpec, Root};
use app_models::{EmbedDeps, ModelsApp, ModelsDeps, ModelsOptions};
use search_impl::ReindexOptions;
use sha2::{Digest, Sha256};

pub fn sha(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// Treść deterministyczna o długości `n`.
pub fn blob(n: usize, seed: u8) -> Vec<u8> {
    (0..n)
        .map(|i| (i as u8).wrapping_mul(31).wrapping_add(seed))
        .collect()
}

/// Pozycja z plikami serwera (`(nazwa, ścieżka URL, treść, przypiąć hash?)`).
pub fn spec(
    id: &str,
    kind: ModelItemKind,
    server: &http::Server,
    files: &[(&str, &str, &[u8], bool)],
    install: Install,
) -> ItemSpec {
    ItemSpec {
        id: id.into(),
        kind,
        name: format!("Pozycja {id}"),
        license: "MIT".into(),
        source: "https://example.invalid".into(),
        root: Root::Models,
        dir: format!("t/{id}"),
        files: files
            .iter()
            .map(|(name, path, bytes, pin)| {
                server.put(path, bytes.to_vec());
                FileSpec {
                    name: (*name).into(),
                    url: format!("{}{path}", server.base),
                    size: bytes.len() as u64,
                    sha256: pin.then(|| sha(bytes)),
                }
            })
            .collect(),
        install,
        confirmed: false,
        note: LocalizedText::new("test", "test"),
    }
}

pub struct Harness {
    pub dir: tempfile::TempDir,
    pub paths: AppPaths,
    pub app: Arc<ModelsApp>,
    pub events: Arc<Mutex<Vec<AlfaEvent>>>,
}

pub fn options(parallel: usize) -> ModelsOptions {
    ModelsOptions {
        parallel,
        loopback_http: true,
        reindex: ReindexOptions {
            batch: 4,
            pause: Duration::ZERO,
        },
        startup_delay: None,
    }
}

/// Menedżer nad katalogiem tymczasowym z dziennikiem zdarzeń.
pub fn harness(catalog: Vec<ItemSpec>, embed: Option<EmbedDeps>, opts: ModelsOptions) -> Harness {
    let dir = tempfile::tempdir().unwrap();
    harness_in(dir, catalog, embed, opts)
}

pub fn harness_in(
    dir: tempfile::TempDir,
    catalog: Vec<ItemSpec>,
    embed: Option<EmbedDeps>,
    opts: ModelsOptions,
) -> Harness {
    let paths = AppPaths::under(dir.path());
    let hub = EventHub::start(Duration::from_millis(1));
    let events = Arc::new(Mutex::new(Vec::new()));
    let mut rx = hub.subscribe();
    let sink = events.clone();
    tokio::spawn(async move {
        while let Ok(batch) = rx.recv().await {
            sink.lock().unwrap().extend(batch.iter().cloned());
        }
    });
    let app = ModelsApp::open(ModelsDeps {
        paths: paths.clone(),
        catalog,
        events: Some(hub),
        embed,
        options: opts,
    });
    Harness {
        dir,
        paths,
        app,
        events,
    }
}

impl Harness {
    pub async fn item(&self, id: &str) -> ModelItem {
        let view = self.app.list().await.unwrap();
        view.items.into_iter().find(|i| i.id == id).unwrap()
    }

    /// Czeka (≤ 10 s), aż pozycja osiągnie stan.
    pub async fn wait(&self, id: &str, state: ModelItemState) -> ModelItem {
        for _ in 0..1000 {
            let item = self.item(id).await;
            if item.state == state {
                return item;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        panic!("{id}: brak stanu {state:?}, jest {:?}", self.item(id).await);
    }

    pub fn events(&self) -> Vec<AlfaEvent> {
        self.events.lock().unwrap().clone()
    }
}
