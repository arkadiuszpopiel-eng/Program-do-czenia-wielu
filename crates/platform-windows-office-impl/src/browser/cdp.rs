//! Połączenie CDP przez potok (`--remote-debugging-pipe`): wiadomości JSON zakończone bajtem `\0`.
//! Wątek czytający rozdziela odpowiedzi (po `id`) i zdarzenia oraz **sam** rozstrzyga
//! `Fetch.requestPaused` filtrem egressu (decyzja bez czekania na wywołującego) i przygotowuje
//! każdy nowo dołączony cel (`Target.attachedToTarget`, wstrzymany do startu): `Fetch.enable`,
//! blokada WebSocketów, autodołączanie potomnych celów, dopiero potem wznowienie. Cel, któremu nie
//! dało się włączyć przechwytywania, jest zamykany (fail-closed).

use std::collections::{HashMap, VecDeque};
use std::io::{BufRead, BufReader, Read, Write};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError, SyncSender};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use platform_apps_contract::{BrowserError, EgressFilter, request_allowed, url_host};
use serde_json::{Value, json};

/// Największa wiadomość CDP (zrzuty ekranu są duże).
const MAX_MESSAGE: usize = 64 * 1024 * 1024;
/// Najwięcej zdarzeń w kolejce (najstarsze odrzucane).
const MAX_EVENTS: usize = 2_000;
/// Wzorce blokowane zawsze (WebSockety omijają domenę `Fetch`).
pub(crate) const ALWAYS_BLOCKED: [&str; 2] = ["ws://*", "wss://*"];

/// Zdarzenie CDP.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Event {
    /// Metoda (`Page.loadEventFired`).
    pub(crate) method: String,
    /// Parametry.
    pub(crate) params: Value,
    /// Sesja celu (`None` = przeglądarka).
    pub(crate) session: Option<String>,
}

type Reply = SyncSender<Result<Value, String>>;

struct Shared {
    writer: Mutex<Box<dyn Write + Send>>,
    next: AtomicU64,
    pending: Mutex<HashMap<u64, Reply>>,
    /// Polecenia wysłane przez wątek czytający przy dołączaniu celu: id → cel do zamknięcia przy błędzie.
    guards: Mutex<HashMap<u64, String>>,
    events: Mutex<VecDeque<Event>>,
    cond: Condvar,
    closed: AtomicBool,
    filter: Arc<dyn EgressFilter>,
    blocked: Mutex<Vec<String>>,
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

impl Shared {
    fn write(&self, id: u64, method: &str, params: Value, session: Option<&str>) -> bool {
        let mut msg = json!({"id": id, "method": method, "params": params});
        if let Some(s) = session {
            msg["sessionId"] = json!(s);
        }
        let mut bytes = msg.to_string().into_bytes();
        bytes.push(0);
        let mut w = lock(&self.writer);
        w.write_all(&bytes).and_then(|()| w.flush()).is_ok()
    }

    fn fire(&self, method: &str, params: Value, session: Option<&str>) -> u64 {
        let id = self.next.fetch_add(1, Ordering::SeqCst);
        self.write(id, method, params, session);
        id
    }

    fn on_paused(&self, params: &Value, session: Option<&str>) {
        let url = params["request"]["url"].as_str().unwrap_or_default();
        let request_id = params["requestId"].clone();
        if request_allowed(self.filter.as_ref(), url) {
            self.fire(
                "Fetch.continueRequest",
                json!({"requestId": request_id}),
                session,
            );
        } else {
            let host = url_host(url).unwrap_or_else(|| url.chars().take(40).collect());
            let mut blocked = lock(&self.blocked);
            if !blocked.contains(&host) && blocked.len() < 200 {
                blocked.push(host);
            }
            drop(blocked);
            self.fire(
                "Fetch.failRequest",
                json!({"requestId": request_id, "errorReason": "BlockedByClient"}),
                session,
            );
        }
    }

    fn on_attached(&self, params: &Value) {
        let Some(session) = params["sessionId"].as_str() else {
            return;
        };
        let target = params["targetInfo"]["targetId"]
            .as_str()
            .unwrap_or_default()
            .to_owned();
        let id = self.fire("Fetch.enable", fetch_patterns(), Some(session));
        lock(&self.guards).insert(id, target);
        self.fire("Network.enable", json!({}), Some(session));
        self.fire(
            "Network.setBlockedURLs",
            json!({"urls": ALWAYS_BLOCKED}),
            Some(session),
        );
        self.fire("Target.setAutoAttach", auto_attach(), Some(session));
        if params["waitingForDebugger"].as_bool() == Some(true) {
            self.fire("Runtime.runIfWaitingForDebugger", json!({}), Some(session));
        }
    }

    fn dispatch(&self, msg: Value) {
        if let Some(id) = msg["id"].as_u64() {
            let reply = match msg.get("error") {
                Some(e) => Err(e["message"].as_str().unwrap_or("błąd CDP").to_owned()),
                None => Ok(msg["result"].clone()),
            };
            if let Some(tx) = lock(&self.pending).remove(&id) {
                let _ = tx.send(reply);
            } else if let Some(target) = lock(&self.guards).remove(&id)
                && reply.is_err()
            {
                self.fire("Target.closeTarget", json!({"targetId": target}), None);
            }
            return;
        }
        let method = msg["method"].as_str().unwrap_or_default().to_owned();
        let session = msg["sessionId"].as_str().map(str::to_owned);
        match method.as_str() {
            "Fetch.requestPaused" => self.on_paused(&msg["params"], session.as_deref()),
            "Target.attachedToTarget" => self.on_attached(&msg["params"]),
            _ => {}
        }
        let mut q = lock(&self.events);
        if q.len() >= MAX_EVENTS {
            q.pop_front();
        }
        q.push_back(Event {
            method,
            params: msg["params"].clone(),
            session,
        });
        drop(q);
        self.cond.notify_all();
    }
}

/// `Fetch.enable` dla wszystkich żądań na etapie wysłania.
pub(crate) fn fetch_patterns() -> Value {
    json!({"patterns": [{"urlPattern": "*", "requestStage": "Request"}]})
}

/// Autodołączanie celów wstrzymanych do startu (ramki OOPIF, okna, workery).
pub(crate) fn auto_attach() -> Value {
    json!({"autoAttach": true, "waitForDebuggerOnStart": true, "flatten": true})
}

/// Połączenie z przeglądarką.
pub(crate) struct Cdp {
    shared: Arc<Shared>,
    timeout: Duration,
}

impl std::fmt::Debug for Cdp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Cdp")
            .field("closed", &self.shared.closed.load(Ordering::SeqCst))
            .finish_non_exhaustive()
    }
}

fn reader_loop(shared: &Shared, reader: Box<dyn Read + Send>) {
    let mut reader = BufReader::new(reader);
    let mut buf = Vec::new();
    loop {
        buf.clear();
        let limit = MAX_MESSAGE as u64 + 1;
        match Read::by_ref(&mut reader)
            .take(limit)
            .read_until(0, &mut buf)
        {
            Ok(0) | Err(_) => break,
            Ok(_) => {}
        }
        if buf.last() != Some(&0) {
            // Wiadomość ponad limit albo urwany strumień — koniec połączenia.
            break;
        }
        buf.pop();
        if let Ok(msg) = serde_json::from_slice::<Value>(&buf) {
            shared.dispatch(msg);
        }
    }
    shared.closed.store(true, Ordering::SeqCst);
    lock(&shared.pending).clear();
    shared.cond.notify_all();
}

impl Cdp {
    /// Połączenie na strumieniach potoku; wątek czytający startuje od razu.
    pub(crate) fn start(
        reader: Box<dyn Read + Send>,
        writer: Box<dyn Write + Send>,
        filter: Arc<dyn EgressFilter>,
        timeout: Duration,
    ) -> Result<Self, BrowserError> {
        let shared = Arc::new(Shared {
            writer: Mutex::new(writer),
            next: AtomicU64::new(1),
            pending: Mutex::new(HashMap::new()),
            guards: Mutex::new(HashMap::new()),
            events: Mutex::new(VecDeque::new()),
            cond: Condvar::new(),
            closed: AtomicBool::new(false),
            filter,
            blocked: Mutex::new(Vec::new()),
        });
        let s = shared.clone();
        std::thread::Builder::new()
            .name("alfa-cdp-reader".into())
            .spawn(move || reader_loop(&s, reader))
            .map_err(|e| BrowserError::Protocol(format!("wątek CDP: {e}")))?;
        Ok(Self { shared, timeout })
    }

    /// Czy połączenie zamknięte.
    pub(crate) fn is_closed(&self) -> bool {
        self.shared.closed.load(Ordering::SeqCst)
    }

    /// Polecenie z odpowiedzią (limit czasu połączenia).
    pub(crate) fn send(
        &self,
        method: &str,
        params: Value,
        session: Option<&str>,
    ) -> Result<Value, BrowserError> {
        if self.is_closed() {
            return Err(BrowserError::Closed("potok CDP zamknięty".into()));
        }
        let id = self.shared.next.fetch_add(1, Ordering::SeqCst);
        let (tx, rx) = mpsc::sync_channel(1);
        lock(&self.shared.pending).insert(id, tx);
        if !self.shared.write(id, method, params, session) {
            lock(&self.shared.pending).remove(&id);
            return Err(BrowserError::Closed("zapis do potoku CDP".into()));
        }
        match rx.recv_timeout(self.timeout) {
            Ok(Ok(v)) => Ok(v),
            Ok(Err(m)) => Err(BrowserError::Protocol(format!("{method}: {m}"))),
            Err(RecvTimeoutError::Timeout) => {
                lock(&self.shared.pending).remove(&id);
                Err(BrowserError::Timeout {
                    op: method.to_owned(),
                    ms: u64::try_from(self.timeout.as_millis()).unwrap_or(u64::MAX),
                })
            }
            Err(RecvTimeoutError::Disconnected) => Err(BrowserError::Closed(
                "przeglądarka zakończyła działanie".into(),
            )),
        }
    }

    /// Czeka na pierwsze pasujące zdarzenie (usuwa je z kolejki).
    pub(crate) fn wait_event<F>(&self, timeout: Duration, mut pred: F) -> Option<Event>
    where
        F: FnMut(&Event) -> bool,
    {
        let deadline = Instant::now() + timeout;
        let mut q = lock(&self.shared.events);
        loop {
            if let Some(i) = q.iter().position(&mut pred) {
                return q.remove(i);
            }
            let now = Instant::now();
            if now >= deadline || self.is_closed() {
                return None;
            }
            q = self
                .shared
                .cond
                .wait_timeout(q, deadline - now)
                .map_or_else(|p| p.into_inner().0, |(g, _)| g);
        }
    }

    /// Zabiera pasujące zdarzenia z kolejki.
    pub(crate) fn drain<F>(&self, mut pred: F) -> Vec<Event>
    where
        F: FnMut(&Event) -> bool,
    {
        let mut q = lock(&self.shared.events);
        let (taken, kept): (Vec<Event>, Vec<Event>) = q.drain(..).partition(|e| pred(e));
        q.extend(kept);
        taken
    }

    /// Hosty zablokowane od ostatniego wywołania.
    pub(crate) fn take_blocked(&self) -> Vec<String> {
        std::mem::take(&mut *lock(&self.shared.blocked))
    }
}
