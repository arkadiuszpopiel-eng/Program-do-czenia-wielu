//! Atrapa potoków z ACL i tożsamości procesów: potoki w pamięci z symulacją DACL (konto klienta
//! musi być właścicielem albo na liście klientów), etykiety integralności (no-write-up) i ochrony
//! pierwszej instancji; rejestr tożsamości po PID ustawiany przez test („proces o innym SID”).

use std::collections::{BTreeMap, VecDeque};
use std::io::{self, Read, Write};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::time::Duration;

use platform_contract::{
    PeerIdentity, PipeConnection, PipeListener, PipeSecurity, PlatformError, ProcessIdentityPort,
    SecurePipePort, Sid,
};

/// Ile atrapa czeka na dane/klienta, zanim zgłosi błąd (chroni testy przed zawieszeniem).
const WAIT_LIMIT: Duration = Duration::from_secs(10);

#[derive(Debug, Default)]
struct Chan {
    buf: VecDeque<u8>,
    closed: bool,
}

type Shared<T> = Arc<(Mutex<T>, Condvar)>;

fn lock<T>(s: &Shared<T>) -> MutexGuard<'_, T> {
    s.0.lock().unwrap_or_else(|p| p.into_inner())
}

/// Jeden koniec połączenia w pamięci.
#[derive(Debug)]
pub struct FakePipeConnection {
    rx: Shared<Chan>,
    tx: Shared<Chan>,
    peer_pid: u32,
}

impl Read for FakePipeConnection {
    fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
        let guard = lock(&self.rx);
        let (mut chan, timeout) = self
            .rx
            .1
            .wait_timeout_while(guard, WAIT_LIMIT, |c| c.buf.is_empty() && !c.closed)
            .unwrap_or_else(|p| p.into_inner());
        if timeout.timed_out() {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "atrapa potoku: brak danych",
            ));
        }
        let n = out.len().min(chan.buf.len());
        for (dst, src) in out.iter_mut().zip(chan.buf.drain(..n)) {
            *dst = src;
        }
        Ok(n)
    }
}

impl Write for FakePipeConnection {
    fn write(&mut self, data: &[u8]) -> io::Result<usize> {
        let mut chan = lock(&self.tx);
        if chan.closed {
            return Err(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "druga strona zamknięta",
            ));
        }
        chan.buf.extend(data);
        self.tx.1.notify_all();
        Ok(data.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl Drop for FakePipeConnection {
    fn drop(&mut self) {
        for side in [&self.tx, &self.rx] {
            lock(side).closed = true;
            side.1.notify_all();
        }
    }
}

impl PipeConnection for FakePipeConnection {
    fn peer_pid(&self) -> u32 {
        self.peer_pid
    }
}

#[derive(Debug)]
struct Slot {
    security: PipeSecurity,
    server_pid: u32,
    queue: VecDeque<FakePipeConnection>,
}

#[derive(Debug, Default)]
struct State {
    pipes: BTreeMap<String, Slot>,
    identities: BTreeMap<u32, PeerIdentity>,
    rejected: Vec<(u32, String)>,
}

/// Potoki i tożsamości w pamięci. Klony dzielą stan; [`FakePipes::process`] daje widok innego
/// procesu (PID wywołującego dla `listen`/`connect`).
#[derive(Debug, Clone, Default)]
pub struct FakePipes {
    state: Shared<State>,
    pid: u32,
}

impl FakePipes {
    /// Nowy „system” z procesem wywołującym `pid` o tożsamości `identity`.
    pub fn new(identity: PeerIdentity) -> Self {
        let pid = identity.pid;
        let me = Self {
            state: Shared::default(),
            pid,
        };
        me.register(identity);
        me
    }

    /// Rejestruje (albo podmienia) tożsamość procesu.
    pub fn register(&self, identity: PeerIdentity) {
        lock(&self.state).identities.insert(identity.pid, identity);
    }

    /// Widok tego samego systemu z perspektywy procesu `pid`.
    pub fn process(&self, pid: u32) -> Self {
        Self {
            state: self.state.clone(),
            pid,
        }
    }

    /// Odrzucone próby połączenia (PID, powód) — do asercji testów.
    pub fn rejected(&self) -> Vec<(u32, String)> {
        lock(&self.state).rejected.clone()
    }

    fn me(&self, st: &State) -> Result<PeerIdentity, PlatformError> {
        st.identities
            .get(&self.pid)
            .cloned()
            .ok_or_else(|| PlatformError::UnknownResource(format!("proces {}", self.pid)))
    }
}

impl ProcessIdentityPort for FakePipes {
    fn identify(&self, pid: u32) -> Result<PeerIdentity, PlatformError> {
        lock(&self.state)
            .identities
            .get(&pid)
            .cloned()
            .ok_or_else(|| PlatformError::UnknownResource(format!("proces {pid}")))
    }

    fn current_user(&self) -> Result<Sid, PlatformError> {
        let st = lock(&self.state);
        self.me(&st).map(|i| i.user)
    }
}

/// Nasłuch atrapy.
#[derive(Debug)]
pub struct FakePipeListener {
    pipes: FakePipes,
    name: String,
}

impl PipeListener for FakePipeListener {
    fn accept(&mut self) -> Result<Box<dyn PipeConnection>, PlatformError> {
        let guard = lock(&self.pipes.state);
        let (mut st, timeout) = self
            .pipes
            .state
            .1
            .wait_timeout_while(guard, WAIT_LIMIT, |st| {
                st.pipes.get(&self.name).is_some_and(|s| s.queue.is_empty())
            })
            .unwrap_or_else(|p| p.into_inner());
        if timeout.timed_out() {
            return Err(PlatformError::Io("atrapa potoku: brak klienta".into()));
        }
        st.pipes
            .get_mut(&self.name)
            .and_then(|s| s.queue.pop_front())
            .map(|c| Box::new(c) as Box<dyn PipeConnection>)
            .ok_or_else(|| PlatformError::Io("potok zamknięty".into()))
    }
}

impl Drop for FakePipeListener {
    fn drop(&mut self) {
        lock(&self.pipes.state).pipes.remove(&self.name);
        self.pipes.state.1.notify_all();
    }
}

impl SecurePipePort for FakePipes {
    fn listen(&self, security: &PipeSecurity) -> Result<Box<dyn PipeListener>, PlatformError> {
        let mut st = lock(&self.state);
        let me = self.me(&st)?;
        if me.user != *security.owner() {
            return Err(PlatformError::PermissionDenied(
                "serwer działa na innym koncie niż właściciel potoku".into(),
            ));
        }
        if st.pipes.contains_key(security.name()) {
            return Err(PlatformError::PermissionDenied(format!(
                "{}: pierwsza instancja zajęta (możliwe przejęcie nazwy)",
                security.path()
            )));
        }
        let slot = Slot {
            security: security.clone(),
            server_pid: self.pid,
            queue: VecDeque::new(),
        };
        st.pipes.insert(security.name().to_owned(), slot);
        Ok(Box::new(FakePipeListener {
            pipes: self.clone(),
            name: security.name().to_owned(),
        }))
    }

    fn connect(
        &self,
        name: &str,
        _timeout_ms: u32,
    ) -> Result<Box<dyn PipeConnection>, PlatformError> {
        let mut st = lock(&self.state);
        let me = self.me(&st)?;
        let Some(slot) = st.pipes.get_mut(name) else {
            return Err(PlatformError::NotFound(format!(r"\\.\pipe\{name}").into()));
        };
        let sec = &slot.security;
        let allowed = me.user == *sec.owner() || sec.clients().contains(&me.user);
        let reason = if !allowed {
            Some(format!("odmowa dostępu (DACL): konto {}", me.user))
        } else if me.integrity < sec.min_integrity() {
            Some(format!(
                "odmowa dostępu (etykieta): poziom {:?}",
                me.integrity
            ))
        } else {
            None
        };
        if let Some(reason) = reason {
            st.rejected.push((self.pid, reason.clone()));
            return Err(PlatformError::PermissionDenied(reason));
        }
        let (a, b) = (Shared::<Chan>::default(), Shared::<Chan>::default());
        slot.queue.push_back(FakePipeConnection {
            rx: a.clone(),
            tx: b.clone(),
            peer_pid: self.pid,
        });
        let client = FakePipeConnection {
            rx: b,
            tx: a,
            peer_pid: slot.server_pid,
        };
        self.state.1.notify_all();
        Ok(Box::new(client))
    }
}
