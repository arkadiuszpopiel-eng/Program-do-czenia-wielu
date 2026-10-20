//! Zapamiętane obniżenia autonomii a Broker startujący później albo ponownie (audyt 2026-10-14,
//! A-04): obniżenie globalne i obniżenie sesji wracają po ponownym uruchomieniu aplikacji, także
//! gdy Broker odpowiada dopiero po chwili z poziomem domyślnym. Podniesień strażnik nie przywraca,
//! ale podniesienia zatwierdzonego w oknie Brokera też nie cofa (zgłoszenie 2026-10-20).

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use app_core::dto::{
    AutonomyLevel, BrokerIntentResult, BrokerIntentStatus, BrokerStatusView, SessionTemplate,
};
use app_core::ports::{AutonomyView, BrokerPort, HeadlessShell};
use app_core::{AppCore, AppError, AppPaths, MemorySecretStore};
use async_trait::async_trait;
use common::options;
use sessions_contract::SessionId;

/// Broker w pamięci: poziom globalny i sesji; `ready = false` — jeszcze nie odpowiada.
#[derive(Default)]
struct Broker {
    ready: AtomicBool,
    /// Podniesienie czeka na zgodę w oknie Brokera (jak prawdziwy Broker).
    window: AtomicBool,
    global: Mutex<AutonomyLevel>,
    sessions: Mutex<BTreeMap<String, AutonomyLevel>>,
    requests: Mutex<Vec<(AutonomyLevel, Option<String>)>>,
}

#[async_trait]
impl BrokerPort for Broker {
    async fn levels(&self, session: Option<&SessionId>) -> Option<AutonomyView> {
        if !self.ready.load(Ordering::SeqCst) {
            return None;
        }
        Some(AutonomyView {
            global: *self.global.lock().unwrap(),
            session: session.and_then(|s| self.sessions.lock().unwrap().get(s.as_str()).copied()),
        })
    }
    async fn request_level(
        &self,
        level: AutonomyLevel,
        session: Option<&SessionId>,
    ) -> Result<BrokerIntentResult, AppError> {
        self.requests
            .lock()
            .unwrap()
            .push((level, session.map(|s| s.as_str().to_owned())));
        if self.window.load(Ordering::SeqCst) && level > *self.global.lock().unwrap() {
            return Ok(BrokerIntentResult {
                status: BrokerIntentStatus::OpenedBroker,
                request_id: "okno".into(),
            });
        }
        match session {
            Some(s) => {
                self.sessions
                    .lock()
                    .unwrap()
                    .insert(s.as_str().to_owned(), level);
            }
            None => *self.global.lock().unwrap() = level,
        }
        Ok(BrokerIntentResult {
            status: BrokerIntentStatus::Applied,
            request_id: "test".into(),
        })
    }
    async fn open_approval(&self, _approval: &str) -> Result<BrokerIntentResult, AppError> {
        Err(AppError::invalid("nieużywane"))
    }
    async fn run_code(
        &self,
        _session: &SessionId,
        _lang: Option<&str>,
        _code: &str,
    ) -> Result<BrokerIntentResult, AppError> {
        Err(AppError::invalid("nieużywane"))
    }
    async fn undo_step(&self, _session: &SessionId, _step: u64) -> Result<String, AppError> {
        Err(AppError::invalid("nieużywane"))
    }
    fn status(&self) -> BrokerStatusView {
        BrokerStatusView::unavailable()
    }
}

/// Ten sam magazyn sekretów w obu uruchomieniach (klucz bazy danych jak w prawdziwej aplikacji).
async fn start(
    dir: &std::path::Path,
    broker: Arc<Broker>,
    secrets: &Arc<MemorySecretStore>,
) -> AppCore {
    let mut opts = options(None, Arc::new(HeadlessShell::default()));
    opts.broker = Some(broker);
    opts.secrets = Some(secrets.clone());
    AppCore::build(AppPaths::under(dir), opts).await.unwrap()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn lowered_levels_come_back_when_a_late_broker_starts_with_defaults() {
    let dir = tempfile::tempdir().unwrap();
    let secrets = Arc::new(MemorySecretStore::default());

    // Pierwsze uruchomienie: właściciel obniża poziom globalny do L1, a jednej sesji do L0.
    let first = Arc::new(Broker::default());
    first.ready.store(true, Ordering::SeqCst);
    let core = start(dir.path(), first.clone(), &secrets).await;
    let session = core
        .sessions_create(SessionTemplate::Empty)
        .await
        .unwrap()
        .id;
    core.permissions_request_level(AutonomyLevel::L1, None)
        .await
        .unwrap();
    core.permissions_request_level(AutonomyLevel::L0, Some(session.clone()))
        .await
        .unwrap();
    drop(core);

    // Drugie uruchomienie: Broker odpowiada dopiero po chwili, z poziomem domyślnym L3.
    let late = Arc::new(Broker::default());
    let core = start(dir.path(), late.clone(), &secrets).await;
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert!(
        late.requests.lock().unwrap().is_empty(),
        "Broker nieosiągalny — nic nie wysłano"
    );
    late.ready.store(true, Ordering::SeqCst);
    let mut waited = Duration::ZERO;
    while late.requests.lock().unwrap().len() < 2 && waited < Duration::from_secs(10) {
        tokio::time::sleep(Duration::from_millis(100)).await;
        waited += Duration::from_millis(100);
    }
    let requests = late.requests.lock().unwrap().clone();
    assert!(
        requests.contains(&(AutonomyLevel::L1, None)),
        "{requests:?}"
    );
    assert!(
        requests.contains(&(AutonomyLevel::L0, Some(session.clone()))),
        "{requests:?}"
    );
    assert_eq!(*late.global.lock().unwrap(), AutonomyLevel::L1);

    // Kolejne sprawdzenie nic nie zmienia — Broker ma już poziomy niższe albo równe.
    late.requests.lock().unwrap().clear();
    tokio::time::sleep(Duration::from_millis(2_500)).await;
    assert!(late.requests.lock().unwrap().is_empty());
    drop(core);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn raise_approved_in_broker_window_is_kept() {
    let dir = tempfile::tempdir().unwrap();
    let secrets = Arc::new(MemorySecretStore::default());
    let broker = Arc::new(Broker::default());
    broker.ready.store(true, Ordering::SeqCst);
    broker.window.store(true, Ordering::SeqCst);
    *broker.global.lock().unwrap() = AutonomyLevel::L3;
    let core = start(dir.path(), broker.clone(), &secrets).await;
    // Obniżenie do L2 (zapamiętane), potem prośba o L4 — karta w oknie Brokera.
    core.permissions_request_level(AutonomyLevel::L2, None)
        .await
        .unwrap();
    let asked = core
        .permissions_request_level(AutonomyLevel::L4, None)
        .await
        .unwrap();
    assert_eq!(asked.status, BrokerIntentStatus::OpenedBroker);
    // Właściciel zatwierdza w oknie: Broker ma L4.
    *broker.global.lock().unwrap() = AutonomyLevel::L4;
    drop(core);
    broker.requests.lock().unwrap().clear();

    // Strażnik (start aplikacji) przyjmuje L4 zamiast cofać do zapamiętanego L2.
    let core = start(dir.path(), broker.clone(), &secrets).await;
    tokio::time::sleep(Duration::from_millis(500)).await;
    assert!(
        broker.requests.lock().unwrap().is_empty(),
        "{:?}",
        broker.requests.lock().unwrap()
    );
    assert_eq!(*broker.global.lock().unwrap(), AutonomyLevel::L4);
    drop(core);

    // Restart Brokera z poziomem domyślnym (L3) po przyjęciu L4 — L4 jest teraz zapamiętane,
    // więc strażnik niczego nie obniża; obniżenia nadal działają.
    *broker.global.lock().unwrap() = AutonomyLevel::L3;
    let core = start(dir.path(), broker.clone(), &secrets).await;
    tokio::time::sleep(Duration::from_millis(500)).await;
    assert!(broker.requests.lock().unwrap().is_empty());
    core.permissions_request_level(AutonomyLevel::L1, None)
        .await
        .unwrap();
    assert_eq!(*broker.global.lock().unwrap(), AutonomyLevel::L1);
    drop(core);
}
