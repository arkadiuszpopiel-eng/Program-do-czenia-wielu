//! Sterownik produkcyjny: zegar, nadzór z magistrali, przegląd co minutę, raport dzienny wg
//! crona w strefie nadzoru, publikacja zdarzeń, księga reguł w magazynie.

use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard, Weak};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use core_bus_contract::{BusItem, Event, EventBus, EventFilter, EventStream};
use futures_util::StreamExt;
use marshal_contract::{
    Ceiling, Marshal, MarshalCore, MarshalHost, RuleBook, RuleTranslator, Watch, WatchConfig,
};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio::time::Instant;
use triggers_contract::CronExpr;

/// Co ile sprawdzać długie blokady.
pub const CHECK_EVERY_MS: u64 = 60_000;

pub(crate) fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

fn millis(d: Duration) -> u64 {
    u64::try_from(d.as_millis()).unwrap_or(u64::MAX)
}

/// Magazyn księgi reguł.
pub trait MarshalStore: Send + Sync {
    /// Ostatnio zapisana księga.
    fn load(&self) -> Result<Option<RuleBook>, String>;
    /// Zapis.
    fn save(&self, book: &RuleBook) -> Result<(), String>;
}

/// Księga w pamięci.
#[derive(Debug, Default)]
pub struct MemMarshalStore(Mutex<Option<RuleBook>>);

impl MarshalStore for MemMarshalStore {
    fn load(&self) -> Result<Option<RuleBook>, String> {
        Ok(lock(&self.0).clone())
    }

    fn save(&self, book: &RuleBook) -> Result<(), String> {
        *lock(&self.0) = Some(book.clone());
        Ok(())
    }
}

/// Księga w pliku JSON (`%LOCALAPPDATA%\Alfa\marshal\rules.json`), zapis przez plik tymczasowy.
#[derive(Debug)]
pub struct FileMarshalStore {
    path: PathBuf,
    write: Mutex<()>,
}

impl FileMarshalStore {
    /// Magazyn w pliku `path`.
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            write: Mutex::new(()),
        }
    }
}

impl MarshalStore for FileMarshalStore {
    fn load(&self) -> Result<Option<RuleBook>, String> {
        match std::fs::read(&self.path) {
            Ok(bytes) => serde_json::from_slice(&bytes)
                .map(Some)
                .map_err(|e| format!("uszkodzona księga reguł: {e}")),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(format!("odczyt księgi reguł: {e}")),
        }
    }

    fn save(&self, book: &RuleBook) -> Result<(), String> {
        let _guard = lock(&self.write);
        if let Some(dir) = self.path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| format!("katalog księgi: {e}"))?;
        }
        let bytes = serde_json::to_vec(book).map_err(|e| e.to_string())?;
        let tmp = self.path.with_extension("json.tmp");
        std::fs::write(&tmp, bytes).map_err(|e| format!("zapis księgi: {e}"))?;
        std::fs::rename(&tmp, &self.path).map_err(|e| format!("zapis księgi: {e}"))
    }
}

/// Tłumacz nieskonfigurowany (bez LLM): propozycje tylko z edytora reguł.
#[derive(Debug, Default, Clone, Copy)]
pub struct NoTranslator;

#[async_trait::async_trait]
impl RuleTranslator for NoTranslator {
    async fn translate(
        &self,
        _text: &str,
        _ceiling: &Ceiling,
    ) -> Result<Vec<serde_json::Value>, String> {
        Err("tłumacz poleceń nie jest skonfigurowany".into())
    }
}

/// Otoczenie produkcyjne.
pub struct ImplHost {
    epoch_ms: u64,
    start: Instant,
    events: mpsc::UnboundedSender<Vec<Event>>,
    store: Arc<dyn MarshalStore>,
}

impl MarshalHost for ImplHost {
    fn now_ms(&self) -> u64 {
        self.epoch_ms.saturating_add(millis(self.start.elapsed()))
    }

    fn emit(&self, events: Vec<Event>) {
        let _ = self.events.send(events);
    }

    fn persist(&self, book: &RuleBook) {
        // Zmiany reguł są rzadkie (decyzje użytkownika) — zapis od razu; błąd jako zdarzenie.
        if let Err(error) = self.store.save(book) {
            self.emit(vec![marshal_contract::marshal_event(
                "marshal.persist_failed",
                self.now_ms(),
                serde_json::json!({ "error": error }),
            )]);
        }
    }
}

/// Uruchomiony Marszałek.
pub(crate) struct Service {
    pub(crate) core: Arc<MarshalCore<ImplHost>>,
    tasks: Mutex<Vec<JoinHandle<()>>>,
}

/// Ustawienia sterownika.
#[derive(Clone)]
pub(crate) struct Settings {
    pub(crate) watch: WatchConfig,
    pub(crate) report: CronExpr,
    pub(crate) start_ms: Option<u64>,
}

impl Service {
    pub(crate) async fn start(
        bus: Arc<dyn EventBus>,
        translator: Arc<dyn RuleTranslator>,
        store: Arc<dyn MarshalStore>,
        settings: Settings,
    ) -> Result<Arc<Self>, String> {
        let book = store.load()?.unwrap_or_default();
        let (tx, rx) = mpsc::unbounded_channel();
        let epoch_ms = settings.start_ms.unwrap_or_else(|| {
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_or(0, millis)
        });
        let host = Arc::new(ImplHost {
            epoch_ms,
            start: Instant::now(),
            events: tx,
            store,
        });
        let core = Arc::new(MarshalCore::new(
            host,
            translator,
            book,
            Watch::new(settings.watch.clone()),
        ));
        let filter = EventFilter {
            kind_prefixes: vec!["scheduler.".into(), "triggers.".into()],
            ..EventFilter::all()
        };
        let stream = bus.subscribe(filter).await.map_err(|e| e.to_string())?;
        let svc = Arc::new(Self {
            core,
            tasks: Mutex::new(Vec::new()),
        });
        let weak = Arc::downgrade(&svc);
        *lock(&svc.tasks) = vec![
            tokio::spawn(publisher(bus, rx)),
            tokio::spawn(listener(weak.clone(), stream)),
            tokio::spawn(checker(weak.clone())),
            tokio::spawn(reporter(weak, settings)),
        ];
        Ok(svc)
    }

    pub(crate) fn shutdown(&self) {
        for task in lock(&self.tasks).drain(..) {
            task.abort();
        }
    }
}

async fn publisher(bus: Arc<dyn EventBus>, mut rx: mpsc::UnboundedReceiver<Vec<Event>>) {
    while let Some(events) = rx.recv().await {
        for event in events {
            let _ = bus.publish(event).await;
        }
    }
}

async fn listener(svc: Weak<Service>, mut stream: EventStream) {
    while let Some(item) = stream.next().await {
        let BusItem::Event(event) = item else {
            continue;
        };
        let Some(s) = svc.upgrade() else {
            return;
        };
        s.core.observe(&event);
    }
}

async fn checker(svc: Weak<Service>) {
    loop {
        tokio::time::sleep(Duration::from_millis(CHECK_EVERY_MS)).await;
        let Some(s) = svc.upgrade() else {
            return;
        };
        s.core.check();
    }
}

async fn reporter(svc: Weak<Service>, settings: Settings) {
    loop {
        let Some(now) = svc.upgrade().map(|s| s.core.host().now_ms()) else {
            return;
        };
        let Some(at) = settings.report.next_after(now, &settings.watch.tz) else {
            return;
        };
        tokio::time::sleep(Duration::from_millis(at.saturating_sub(now))).await;
        let Some(s) = svc.upgrade() else {
            return;
        };
        let day = i64::try_from(at)
            .ok()
            .and_then(|t| settings.watch.tz.to_local(t))
            .map(|t| t.date());
        if let Some(day) = day {
            s.core.publish_report(day);
        }
    }
}
