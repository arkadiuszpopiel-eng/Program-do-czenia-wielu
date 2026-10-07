//! Potoki serwera, które da się „zerwać” (awaria procesu Brokera): po `break_all` każde
//! połączenie przyjęte przez serwer kończy się błędem przy następnym odczycie — serwer zamyka
//! je, a klient widzi koniec strumienia, dokładnie jak po śmierci usługi.

use std::io::{self, Read, Write};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use platform_contract::{
    PipeConnection, PipeListener, PipeSecurity, PlatformError, SecurePipePort,
};

/// Opakowanie portu serwera.
#[derive(Clone)]
pub struct Breaker {
    inner: Arc<dyn SecurePipePort>,
    broken: Arc<AtomicBool>,
}

impl Breaker {
    pub fn new(inner: Arc<dyn SecurePipePort>) -> Self {
        Self {
            inner,
            broken: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Zrywa wszystkie połączenia (i odmawia nowych do `heal`).
    pub fn break_all(&self) {
        self.broken.store(true, Ordering::SeqCst);
    }

    /// Kolejne połączenia znowu działają.
    pub fn heal(&self) {
        self.broken.store(false, Ordering::SeqCst);
    }
}

struct Listener {
    inner: Box<dyn PipeListener>,
    broken: Arc<AtomicBool>,
}

struct Conn {
    inner: Box<dyn PipeConnection>,
    broken: Arc<AtomicBool>,
}

impl Read for Conn {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let n = self.inner.read(buf)?;
        if self.broken.load(Ordering::SeqCst) {
            return Err(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "Broker przestał działać",
            ));
        }
        Ok(n)
    }
}

impl Write for Conn {
    fn write(&mut self, data: &[u8]) -> io::Result<usize> {
        if self.broken.load(Ordering::SeqCst) {
            return Err(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "Broker przestał działać",
            ));
        }
        self.inner.write(data)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

impl PipeConnection for Conn {
    fn peer_pid(&self) -> u32 {
        self.inner.peer_pid()
    }
}

impl PipeListener for Listener {
    fn accept(&mut self) -> Result<Box<dyn PipeConnection>, PlatformError> {
        let conn = self.inner.accept()?;
        Ok(Box::new(Conn {
            inner: conn,
            broken: self.broken.clone(),
        }))
    }
}

impl SecurePipePort for Breaker {
    fn listen(&self, security: &PipeSecurity) -> Result<Box<dyn PipeListener>, PlatformError> {
        let inner = self.inner.listen(security)?;
        Ok(Box::new(Listener {
            inner,
            broken: self.broken.clone(),
        }))
    }

    fn connect(
        &self,
        name: &str,
        timeout_ms: u32,
    ) -> Result<Box<dyn PipeConnection>, PlatformError> {
        self.inner.connect(name, timeout_ms)
    }
}
