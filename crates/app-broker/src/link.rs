//! Łącze z Brokerem przez potok z ACL (`SecurePipePort`), protokół IPC z
//! `safety-broker-contract::ipc` w roli `Core` (jądro Alfy przekazuje żądania agentek i UI).
//!
//! - Przed powitaniem klient sprawdza, **kto** jest serwerem potoku ([`ServerCheck`]): w trybie
//!   przenośnym — PID procesu potomnego uruchomionego przez aplikację (inny proces tego samego
//!   konta może utworzyć kolejną instancję potoku), w trybie usługi — sesja 0 i konto usługi.
//! - Połączenie obsługuje osobny wątek (protokół żądanie → odpowiedź, jedno połączenie); wołający
//!   czekają z limitem czasu. Przekroczenie limitu, błąd strumienia albo zamknięcie potoku =
//!   łącze zerwane ([`LinkState::Lost`]) — od tej chwili każde wywołanie kończy się błędem, a
//!   [`crate::RemoteBroker`] zamienia go w odmowę (fail-closed). Ponowne połączenie — nadzór.
//! - Nowe połączenie najpierw odtwarza stan zawężający z [`Journal`] (SR3-03).

use std::sync::mpsc::{self, RecvTimeoutError};
use std::sync::{Arc, Mutex, MutexGuard, Weak};
use std::time::Duration;

use platform_contract::{IntegrityLevel, PipeConnection, ProcessIdentityPort, SecurePipePort, Sid};
use safety_broker_contract::ipc::{
    ClientCredential, ClientRole, Hello, PROTOCOL_VERSION, Request, Response,
};
use safety_broker_contract::ipc_blocking::{BlockingClient, BlockingError};
use tokio::sync::oneshot;

use crate::replay::Journal;
use crate::status::{KernelStatus, LinkState};

/// Kim musi być serwer potoku (ochrona przed podstawionym serwerem).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ServerCheck {
    /// Tryb przenośny: proces `alfa-broker --console` uruchomiony przez aplikację.
    Pid(u32),
    /// Usługa `AlfaBroker`: sesja 0 (usługi), integralność systemowa, konto z `broker.json`.
    Service {
        /// Konto usługi (`None` — konfiguracja bez konta: tylko sesja i integralność).
        user: Option<Sid>,
    },
}

/// Sprawdza serwer potoku; błąd = połączenie odrzucone przed wysłaniem czegokolwiek.
pub fn verify_server(
    identity: &dyn ProcessIdentityPort,
    pid: u32,
    check: &ServerCheck,
) -> Result<(), String> {
    match check {
        ServerCheck::Pid(expected) if pid == *expected => Ok(()),
        ServerCheck::Pid(expected) => Err(format!(
            "serwer potoku to proces {pid}, a nie uruchomiony Broker {expected} — możliwe podstawienie"
        )),
        ServerCheck::Service { user } => {
            let peer = identity
                .identify(pid)
                .map_err(|e| format!("nie można potwierdzić tożsamości serwera potoku: {e}"))?;
            if peer.session != 0 || peer.integrity < IntegrityLevel::System {
                return Err(format!(
                    "serwer potoku (sesja {}, integralność {:?}) nie jest usługą — możliwe podstawienie",
                    peer.session, peer.integrity
                ));
            }
            match user {
                Some(expected) if peer.user != *expected => Err(format!(
                    "serwer potoku działa na koncie {} zamiast {expected} — możliwe podstawienie",
                    peer.user
                )),
                _ => Ok(()),
            }
        }
    }
}

/// Błąd łącza (nie odpowiedź Brokera).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LinkError {
    /// Brak połączenia (zerwane albo jeszcze nie nawiązane).
    Lost(String),
    /// Broker nie odpowiedział w limicie — łącze uznane za zerwane.
    Timeout(u64),
    /// Broker odrzucił powitanie albo serwer nie przeszedł sprawdzenia.
    Rejected(String),
}

impl std::fmt::Display for LinkError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Lost(why) => write!(f, "brak połączenia z Brokerem: {why}"),
            Self::Timeout(ms) => write!(f, "Broker nie odpowiedział w ciągu {ms} ms"),
            Self::Rejected(why) => write!(f, "połączenie odrzucone: {why}"),
        }
    }
}

impl std::error::Error for LinkError {}

/// Parametry łącza.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkConfig {
    /// Nazwa potoku (bez `\\.\pipe\`).
    pub pipe: String,
    /// Ile czekać na wolną instancję potoku (ms).
    pub connect_timeout_ms: u32,
    /// Limit odpowiedzi na żądanie (także na powitanie).
    pub call_timeout: Duration,
}

impl LinkConfig {
    /// Domyślne limity: 1 s na instancję potoku, 3 s na odpowiedź.
    pub fn new(pipe: impl Into<String>) -> Self {
        Self {
            pipe: pipe.into(),
            connect_timeout_ms: 1_000,
            call_timeout: Duration::from_secs(3),
        }
    }
}

type Answer = Result<Response, LinkError>;

enum Reply {
    Sync(mpsc::SyncSender<Answer>),
    Async(oneshot::Sender<Answer>),
}

impl Reply {
    fn send(self, answer: Answer) {
        // Wołający mógł już zrezygnować (limit czasu) — odpowiedź przepada.
        match self {
            Self::Sync(tx) => {
                let _ = tx.send(answer);
            }
            Self::Async(tx) => {
                let _ = tx.send(answer);
            }
        }
    }
}

struct Job {
    request: Request,
    reply: Reply,
}

struct Current {
    generation: u64,
    jobs: Option<mpsc::Sender<Job>>,
}

/// Łącze z Brokerem (jedno połączenie, wątek obsługi, limity czasu, stan w [`KernelStatus`]).
pub struct BrokerLink {
    pipes: Arc<dyn SecurePipePort>,
    identity: Arc<dyn ProcessIdentityPort>,
    config: LinkConfig,
    status: Arc<KernelStatus>,
    current: Mutex<Current>,
    journal: Arc<Journal>,
    me: Weak<BrokerLink>,
}

impl std::fmt::Debug for BrokerLink {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BrokerLink")
            .field("config", &self.config)
            .field("state", &self.status.link())
            .finish_non_exhaustive()
    }
}

fn hello() -> Hello {
    // Jądro przedstawia się bez MAC: Broker przyjmuje rolę `Core` wyłącznie od procesu, którego
    // obraz, konto i integralność pasują do wiązania roli (zapis po tożsamości obrazu).
    Hello {
        protocol: PROTOCOL_VERSION,
        credential: ClientCredential {
            client_id: "core".into(),
            role: ClientRole::Core,
            expires_at_ms: 0,
            mac: String::new(),
        },
        pid: std::process::id(),
        sid: None,
        image: None,
    }
}

fn millis(d: Duration) -> u64 {
    u64::try_from(d.as_millis()).unwrap_or(u64::MAX)
}

impl BrokerLink {
    /// Łącze (jeszcze bez połączenia — [`Self::connect`]).
    pub fn new(
        pipes: Arc<dyn SecurePipePort>,
        identity: Arc<dyn ProcessIdentityPort>,
        config: LinkConfig,
        status: Arc<KernelStatus>,
    ) -> Arc<Self> {
        Arc::new_cyclic(|me| Self {
            pipes,
            identity,
            config,
            status,
            current: Mutex::new(Current {
                generation: 0,
                jobs: None,
            }),
            journal: Arc::default(),
            me: me.clone(),
        })
    }

    fn lock(&self) -> MutexGuard<'_, Current> {
        self.current.lock().unwrap_or_else(|p| p.into_inner())
    }

    /// Stan wspólny (UI, zdrowie).
    pub fn status(&self) -> &Arc<KernelStatus> {
        &self.status
    }

    /// Parametry łącza.
    pub fn config(&self) -> &LinkConfig {
        &self.config
    }

    /// Dziennik stanu zawężającego odtwarzanego na każdym nowym połączeniu.
    pub fn journal(&self) -> &Arc<Journal> {
        &self.journal
    }

    fn open(
        pipes: &dyn SecurePipePort,
        identity: &dyn ProcessIdentityPort,
        config: &LinkConfig,
        check: &ServerCheck,
        replay: Vec<Request>,
    ) -> Result<BlockingClient<Box<dyn PipeConnection>>, LinkError> {
        let conn = pipes
            .connect(&config.pipe, config.connect_timeout_ms)
            .map_err(|e| LinkError::Lost(format!("potok {}: {e}", config.pipe)))?;
        verify_server(identity, conn.peer_pid(), check).map_err(LinkError::Rejected)?;
        let mut client = BlockingClient::connect(conn, &hello()).map_err(|e| match e {
            BlockingError::Rejected(why) => LinkError::Rejected(why),
            other => LinkError::Lost(other.to_string()),
        })?;
        // Przed udostępnieniem połączenia: błąd strumienia = połączenie nieudane, odmowa Brokera
        // (np. obniżenie „na czas” już wygasłe) — tylko wpis w dzienniku.
        for request in replay {
            match client.call(request) {
                Ok(Response::Error(e)) => {
                    tracing::warn!(error = %e, "odtworzenie stanu w Brokerze odrzucone");
                }
                Ok(_) => {}
                Err(e) => {
                    return Err(LinkError::Lost(format!(
                        "odtwarzanie stanu w nowym połączeniu: {e}"
                    )));
                }
            }
        }
        Ok(client)
    }

    /// Łączy się (sprawdzenie serwera, powitanie) z limitem czasu; sukces podmienia połączenie.
    /// Powitanie biegnie w osobnym wątku — zawieszony serwer nie blokuje wołającego dłużej niż limit.
    pub fn connect(&self, check: &ServerCheck) -> Result<(), LinkError> {
        let (tx, rx) = mpsc::sync_channel(1);
        let (pipes, identity) = (self.pipes.clone(), self.identity.clone());
        let (config, check) = (self.config.clone(), check.clone());
        let replay = self.journal.requests();
        std::thread::Builder::new()
            .name("alfa-broker-connect".into())
            .spawn(move || {
                let _ = tx.send(Self::open(&*pipes, &*identity, &config, &check, replay));
            })
            .map_err(|e| LinkError::Lost(format!("wątek łączenia: {e}")))?;
        let client = match rx.recv_timeout(self.config.call_timeout) {
            Ok(result) => result?,
            Err(_) => return Err(LinkError::Timeout(millis(self.config.call_timeout))),
        };
        self.install(client)
    }

    fn install(&self, client: BlockingClient<Box<dyn PipeConnection>>) -> Result<(), LinkError> {
        let (tx, rx) = mpsc::channel::<Job>();
        let generation = {
            let mut cur = self.lock();
            cur.generation += 1;
            cur.jobs = Some(tx);
            cur.generation
        };
        let me = self.me.clone();
        let spawned = std::thread::Builder::new()
            .name("alfa-broker-link".into())
            .spawn(move || worker(client, &rx, &me, generation));
        if let Err(e) = spawned {
            let why = format!("wątek łącza: {e}");
            self.mark_lost(generation, &why);
            return Err(LinkError::Lost(why));
        }
        self.status.set_link(LinkState::Connected);
        Ok(())
    }

    /// Łącze zerwane (jeśli `generation` to nadal bieżące połączenie).
    fn mark_lost(&self, generation: u64, why: &str) {
        let mut cur = self.lock();
        if cur.generation != generation || cur.jobs.is_none() {
            return;
        }
        cur.jobs = None;
        drop(cur);
        tracing::error!(powod = why, "łącze z Brokerem zerwane — bezpieczny stan");
        self.status.set_link(LinkState::Lost(why.to_owned()));
    }

    /// Zrywa bieżące połączenie (zamknięcie aplikacji, testy).
    pub fn disconnect(&self, why: &str) {
        let generation = self.lock().generation;
        self.mark_lost(generation, why);
    }

    fn submit(&self, request: Request, reply: Reply) -> Result<u64, LinkError> {
        let cur = self.lock();
        let lost = || match self.status.link() {
            LinkState::Lost(why) => LinkError::Lost(why),
            _ => LinkError::Lost("łączenie w toku".into()),
        };
        let Some(jobs) = cur.jobs.as_ref() else {
            return Err(lost());
        };
        jobs.send(Job { request, reply }).map_err(|_| lost())?;
        Ok(cur.generation)
    }

    /// Wywołanie blokujące z limitem `call_timeout` (metody synchroniczne `Broker`).
    pub fn call(&self, request: Request) -> Result<Response, LinkError> {
        self.call_within(request, self.config.call_timeout)
    }

    /// Wywołanie blokujące z własnym limitem.
    pub fn call_within(&self, request: Request, limit: Duration) -> Result<Response, LinkError> {
        let (tx, rx) = mpsc::sync_channel(1);
        let generation = self.submit(request, Reply::Sync(tx))?;
        match rx.recv_timeout(limit) {
            Ok(answer) => answer,
            Err(RecvTimeoutError::Timeout) => {
                self.mark_lost(generation, "Broker nie odpowiada w limicie czasu");
                Err(LinkError::Timeout(millis(limit)))
            }
            Err(RecvTimeoutError::Disconnected) => {
                Err(LinkError::Lost("połączenie zamknięte".into()))
            }
        }
    }

    /// Wywołanie asynchroniczne z limitem `call_timeout` (nie blokuje wątku runtime).
    pub async fn call_async(&self, request: Request) -> Result<Response, LinkError> {
        self.call_async_within(request, self.config.call_timeout)
            .await
    }

    /// Wywołanie asynchroniczne z własnym limitem.
    pub async fn call_async_within(
        &self,
        request: Request,
        limit: Duration,
    ) -> Result<Response, LinkError> {
        let (tx, rx) = oneshot::channel();
        let generation = self.submit(request, Reply::Async(tx))?;
        match tokio::time::timeout(limit, rx).await {
            Ok(Ok(answer)) => answer,
            Ok(Err(_)) => Err(LinkError::Lost("połączenie zamknięte".into())),
            Err(_) => {
                self.mark_lost(generation, "Broker nie odpowiada w limicie czasu");
                Err(LinkError::Timeout(millis(limit)))
            }
        }
    }
}

/// Wątek połączenia: żądania po kolei; pierwszy błąd strumienia kończy połączenie.
fn worker(
    mut client: BlockingClient<Box<dyn PipeConnection>>,
    jobs: &mpsc::Receiver<Job>,
    link: &Weak<BrokerLink>,
    generation: u64,
) {
    while let Ok(job) = jobs.recv() {
        let answer = client
            .call(job.request)
            .map_err(|e| LinkError::Lost(e.to_string()));
        let failed = answer.as_ref().err().map(ToString::to_string);
        job.reply.send(answer);
        if let Some(why) = failed {
            if let Some(link) = link.upgrade() {
                link.mark_lost(generation, &why);
            }
            return;
        }
    }
}
