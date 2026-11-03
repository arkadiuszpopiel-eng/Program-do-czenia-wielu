//! Broker w pamięci dla testów strażnika autonomii: poziom globalny i sesji (widok jak u
//! prawdziwego Brokera — poziom sesji tylko, gdy różni się od globalnego), prośby o podniesienie
//! z identyfikatorami i stanem (zatwierdza test, jak właściciel w oknie Brokera).

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use app_core::dto::{
    AlfaEvent, AutonomyLevel, BrokerIntentResult, BrokerIntentStatus, BrokerStatusView,
};
use app_core::ports::{ApprovalOutcome, AutonomyView, BrokerPort, HeadlessShell};
use app_core::{AppCore, AppError, AppPaths, MemorySecretStore};
use async_trait::async_trait;
use sessions_contract::SessionId;

/// `ready = false` — Broker jeszcze nie odpowiada; `window` — podniesienie czeka na zgodę.
#[derive(Default)]
pub struct Broker {
    pub ready: AtomicBool,
    pub window: AtomicBool,
    pub global: Mutex<AutonomyLevel>,
    pub sessions: Mutex<BTreeMap<String, AutonomyLevel>>,
    pub requests: Mutex<Vec<(AutonomyLevel, Option<String>)>>,
    /// Prośby o podniesienie: identyfikator → stan (brak — nieznana, jak w nowym Brokerze).
    pub approvals: Mutex<BTreeMap<String, ApprovalOutcome>>,
    /// Odczyty poziomu globalnego — każdy przebieg strażnika zaczyna się od jednego.
    pub polls: AtomicUsize,
    next: AtomicU64,
}

impl Broker {
    /// Gotowy Broker z oknem zatwierdzeń i poziomem globalnym `global`.
    pub fn with_window(global: AutonomyLevel) -> Arc<Self> {
        let broker = Self::default();
        broker.ready.store(true, Ordering::SeqCst);
        broker.window.store(true, Ordering::SeqCst);
        *broker.global.lock().unwrap() = global;
        Arc::new(broker)
    }

    fn effective(&self, session: Option<&str>) -> AutonomyLevel {
        let global = *self.global.lock().unwrap();
        session
            .and_then(|s| self.sessions.lock().unwrap().get(s).copied())
            .unwrap_or(global)
    }

    fn set(&self, level: AutonomyLevel, session: Option<&str>) {
        match session {
            Some(s) => {
                self.sessions.lock().unwrap().insert(s.to_owned(), level);
            }
            None => *self.global.lock().unwrap() = level,
        }
    }

    /// Właściciel zatwierdza kartę w oknie Brokera: stan „zatwierdzona” i nowy poziom.
    pub fn approve(&self, approval: &str, level: AutonomyLevel, session: Option<&str>) {
        self.approvals
            .lock()
            .unwrap()
            .insert(approval.to_owned(), ApprovalOutcome::Approved);
        self.set(level, session);
    }

    /// Nowy Broker (restart): poziomy domyślne L3, prośby nieznane.
    pub fn restart(&self) {
        *self.global.lock().unwrap() = AutonomyLevel::L3;
        self.sessions.lock().unwrap().clear();
        self.approvals.lock().unwrap().clear();
    }

    pub fn sent(&self) -> Vec<(AutonomyLevel, Option<String>)> {
        self.requests.lock().unwrap().clone()
    }
}

#[async_trait]
impl BrokerPort for Broker {
    async fn levels(&self, session: Option<&SessionId>) -> Option<AutonomyView> {
        if !self.ready.load(Ordering::SeqCst) {
            return None;
        }
        if session.is_none() {
            self.polls.fetch_add(1, Ordering::SeqCst);
        }
        let global = *self.global.lock().unwrap();
        let session = session
            .map(|s| self.effective(Some(s.as_str())))
            .filter(|l| *l != global);
        Some(AutonomyView {
            global,
            session,
            max_access: None,
        })
    }
    async fn request_level(
        &self,
        level: AutonomyLevel,
        session: Option<&SessionId>,
    ) -> Result<BrokerIntentResult, AppError> {
        let scope = session.map(SessionId::as_str);
        self.requests
            .lock()
            .unwrap()
            .push((level, scope.map(str::to_owned)));
        if self.window.load(Ordering::SeqCst) && level > self.effective(scope) {
            let id = (self.next.fetch_add(1, Ordering::SeqCst) + 1).to_string();
            self.approvals
                .lock()
                .unwrap()
                .insert(id.clone(), ApprovalOutcome::Pending);
            return Ok(BrokerIntentResult {
                status: BrokerIntentStatus::OpenedBroker,
                request_id: id,
            });
        }
        self.set(level, scope);
        Ok(BrokerIntentResult {
            status: BrokerIntentStatus::Applied,
            request_id: String::new(),
        })
    }
    async fn approval_outcome(&self, approval: &str) -> Option<ApprovalOutcome> {
        if !self.ready.load(Ordering::SeqCst) {
            return None;
        }
        Some(
            self.approvals
                .lock()
                .unwrap()
                .get(approval)
                .copied()
                .unwrap_or(ApprovalOutcome::Rejected),
        )
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

/// Rdzeń z tym Brokerem; ten sam magazyn sekretów w kolejnych uruchomieniach (klucz bazy).
pub async fn start(
    dir: &std::path::Path,
    broker: Arc<Broker>,
    secrets: &Arc<MemorySecretStore>,
) -> AppCore {
    let mut opts = super::options(None, Arc::new(HeadlessShell::default()));
    opts.broker = Some(broker);
    opts.secrets = Some(secrets.clone());
    AppCore::build(AppPaths::under(dir), opts).await.unwrap()
}

/// Liczba ogłoszeń `SessionUpdated` od początku dziennika zdarzeń.
pub async fn announced(core: &AppCore) -> usize {
    core.events_resume(0, None)
        .await
        .unwrap()
        .frames
        .iter()
        .flat_map(|f| f.events.iter())
        .filter(|e| matches!(e, AlfaEvent::SessionUpdated { .. }))
        .count()
}

/// Czeka, aż strażnik wykona co najmniej `n` kolejnych przebiegów na tym Brokerze (do 40 s: bez
/// oczekujących próśb przebieg jest co 15 s) — asercja „nic nie wysłano” bez tego mogłaby nie objąć
/// żadnego przebiegu.
pub async fn passes(broker: &Broker, n: usize) {
    let start = broker.polls.load(Ordering::SeqCst);
    for _ in 0..400 {
        if broker.polls.load(Ordering::SeqCst) >= start + n {
            return;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    panic!("strażnik nie wykonał {n} przebiegów w 40 s");
}

/// Czeka (do 10 s) na warunek sprawdzany co 100 ms.
pub async fn eventually<F: std::future::Future<Output = bool>>(
    mut check: impl FnMut() -> F,
) -> bool {
    for _ in 0..100 {
        if check().await {
            return true;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    false
}
