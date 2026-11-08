//! Atrapa pseudokonsoli (`PseudoConsolePort`): procesy w pamięci, wyjście skryptowane
//! (`emit`, echo wejścia), koniec procesu (`exit`), zamknięcie = „zabicie drzewa” (flaga do asercji).
//! `Debug` nie pokazuje treści strumienia (w terminalu logowania mogą być tokeny).

use std::collections::BTreeMap;
use std::fmt;
use std::io::Read;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex, MutexGuard};

use platform_contract::{PlatformError, PseudoConsolePort, PtySession, PtySize, PtySpec};

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

#[derive(Default)]
struct Shared {
    input: Mutex<Vec<u8>>,
    out: Mutex<Option<Sender<Vec<u8>>>>,
    reader: Mutex<Option<Receiver<Vec<u8>>>>,
    size: Mutex<Option<PtySize>>,
    exit: Mutex<Option<i32>>,
    closed: AtomicBool,
    tree_killed: AtomicBool,
    echo: bool,
}

impl Shared {
    fn send(&self, bytes: &[u8]) {
        if bytes.is_empty() {
            return;
        }
        if let Some(tx) = lock(&self.out).as_ref() {
            let _ = tx.send(bytes.to_vec());
        }
    }
}

/// Atrapa portu pseudokonsoli.
#[derive(Default)]
pub struct FakePty {
    sessions: Mutex<BTreeMap<u32, Arc<Shared>>>,
    specs: Mutex<Vec<PtySpec>>,
    banner: Mutex<Vec<u8>>,
    echo: AtomicBool,
    next_pid: Mutex<u32>,
    fail_next: Mutex<Option<PlatformError>>,
}

impl fmt::Debug for FakePty {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FakePty")
            .field("sessions", &lock(&self.sessions).len())
            .finish_non_exhaustive()
    }
}

impl FakePty {
    /// Atrapa z wyjściem początkowym (np. ekran logowania CLI) i echem wejścia.
    pub fn with_banner(banner: &[u8], echo: bool) -> Self {
        let p = Self::default();
        *lock(&p.banner) = banner.to_vec();
        p.echo.store(echo, Ordering::SeqCst);
        p
    }

    fn shared(&self, pid: u32) -> Option<Arc<Shared>> {
        lock(&self.sessions).get(&pid).cloned()
    }

    /// Kolejne wyjście procesu `pid`.
    pub fn emit(&self, pid: u32, bytes: &[u8]) {
        if let Some(s) = self.shared(pid) {
            s.send(bytes);
        }
    }

    /// Proces kończy się kodem (EOF na wyjściu).
    pub fn exit(&self, pid: u32, code: i32) {
        if let Some(s) = self.shared(pid) {
            *lock(&s.exit) = Some(code);
            lock(&s.out).take();
        }
    }

    /// Następne `spawn` zwróci błąd.
    pub fn fail_next(&self, error: PlatformError) {
        *lock(&self.fail_next) = Some(error);
    }

    /// Uruchomione specyfikacje.
    pub fn spawned(&self) -> Vec<PtySpec> {
        lock(&self.specs).clone()
    }

    /// Bajty wpisane do procesu `pid`.
    pub fn input_of(&self, pid: u32) -> Vec<u8> {
        self.shared(pid)
            .map(|s| lock(&s.input).clone())
            .unwrap_or_default()
    }

    /// Czy drzewo procesu `pid` zabito (zamknięcie sesji).
    pub fn tree_killed(&self, pid: u32) -> bool {
        self.shared(pid)
            .is_some_and(|s| s.tree_killed.load(Ordering::SeqCst))
    }

    /// Ostatni rozmiar sesji.
    pub fn size_of(&self, pid: u32) -> Option<PtySize> {
        self.shared(pid).and_then(|s| *lock(&s.size))
    }
}

struct FakeSession {
    pid: u32,
    shared: Arc<Shared>,
}

impl fmt::Debug for FakeSession {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FakeSession")
            .field("pid", &self.pid)
            .finish_non_exhaustive()
    }
}

struct ChannelReader {
    rx: Receiver<Vec<u8>>,
    pending: Vec<u8>,
}

impl Read for ChannelReader {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if self.pending.is_empty() {
            match self.rx.recv() {
                Ok(chunk) => self.pending = chunk,
                Err(_) => return Ok(0),
            }
        }
        let n = buf.len().min(self.pending.len());
        buf[..n].copy_from_slice(&self.pending[..n]);
        self.pending.drain(..n);
        Ok(n)
    }
}

impl PtySession for FakeSession {
    fn pid(&self) -> u32 {
        self.pid
    }

    fn take_output(&self) -> Result<Box<dyn Read + Send>, PlatformError> {
        lock(&self.shared.reader)
            .take()
            .map(|rx| {
                Box::new(ChannelReader {
                    rx,
                    pending: Vec::new(),
                }) as Box<dyn Read + Send>
            })
            .ok_or_else(|| PlatformError::Io("wyjście terminala już pobrane".into()))
    }

    fn write_input(&self, data: &[u8]) -> Result<(), PlatformError> {
        if self.shared.closed.load(Ordering::SeqCst) || lock(&self.shared.exit).is_some() {
            return Err(PlatformError::Io("sesja terminala zamknięta".into()));
        }
        lock(&self.shared.input).extend_from_slice(data);
        if self.shared.echo {
            self.shared.send(data);
        }
        Ok(())
    }

    fn resize(&self, size: PtySize) -> Result<(), PlatformError> {
        size.validate()?;
        *lock(&self.shared.size) = Some(size);
        Ok(())
    }

    fn exit_code(&self) -> Result<Option<i32>, PlatformError> {
        Ok(*lock(&self.shared.exit))
    }

    fn close(&self) -> Result<(), PlatformError> {
        self.shared.closed.store(true, Ordering::SeqCst);
        self.shared.tree_killed.store(true, Ordering::SeqCst);
        lock(&self.shared.exit).get_or_insert(-1);
        lock(&self.shared.out).take();
        Ok(())
    }
}

impl PseudoConsolePort for FakePty {
    fn spawn(&self, spec: &PtySpec) -> Result<Box<dyn PtySession>, PlatformError> {
        if let Some(e) = lock(&self.fail_next).take() {
            return Err(e);
        }
        spec.validate()?;
        let (tx, rx) = channel();
        let shared = Arc::new(Shared {
            out: Mutex::new(Some(tx)),
            reader: Mutex::new(Some(rx)),
            size: Mutex::new(Some(spec.size)),
            echo: self.echo.load(Ordering::SeqCst),
            ..Shared::default()
        });
        shared.send(&lock(&self.banner));
        let pid = {
            let mut next = lock(&self.next_pid);
            *next += 1;
            40_000 + *next
        };
        lock(&self.sessions).insert(pid, shared.clone());
        lock(&self.specs).push(spec.clone());
        Ok(Box::new(FakeSession { pid, shared }))
    }
}
