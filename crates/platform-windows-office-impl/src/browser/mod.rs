//! `BrowserPort` przez CDP na potoku: walidacja konfiguracji (profil Alfy, nigdy profil
//! użytkownika), uruchomienie przez [`Launcher`] (Windows: Edge/Chrome w Job Object z potokami
//! fd 3/4), przygotowanie strony i operacje. Sesje są niezależne; zamknięcie zabija całe drzewo
//! procesów przeglądarki.

mod cdp;
#[cfg(windows)]
mod launch_win;
mod page;

use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use platform_apps_contract::{
    BROWSER_CALL_TIMEOUT_MS, BrowserError, BrowserPort, BrowserSessionId, BrowserSpec,
    EgressFilter, MAX_TYPE_CHARS, PageInfo, PageSnapshot, check_navigation_url, chromium_args,
};

#[cfg(windows)]
pub use launch_win::WinLauncher;

/// Proces przeglądarki (zabicie całego drzewa w `kill` i w `Drop`).
pub trait ProcessGuard: Send {
    /// Kończy przeglądarkę i jej procesy potomne.
    fn kill(&mut self);
}

/// Uruchomiona przeglądarka: końcówki potoku CDP i proces.
pub struct Launched {
    /// Odczyt odpowiedzi i zdarzeń (fd 4 przeglądarki).
    pub reader: Box<dyn Read + Send>,
    /// Zapis poleceń (fd 3 przeglądarki).
    pub writer: Box<dyn Write + Send>,
    /// Proces.
    pub process: Box<dyn ProcessGuard>,
}

/// Uruchamianie przeglądarki z CDP przez potok.
pub trait Launcher: Send + Sync {
    /// Startuje przeglądarkę z argumentami ([`chromium_args`]); profil przygotowany wcześniej.
    fn launch(&self, spec: &BrowserSpec, args: &[String]) -> Result<Launched, BrowserError>;
}

/// Uruchamianie niedostępne (poza Windows).
#[derive(Debug, Default, Clone, Copy)]
pub struct NoLauncher;

impl Launcher for NoLauncher {
    fn launch(&self, _spec: &BrowserSpec, _args: &[String]) -> Result<Launched, BrowserError> {
        Err(BrowserError::NotInstalled(
            "przeglądarka z CDP przez potok wymaga Windows".into(),
        ))
    }
}

/// Konfiguracja (`[platform.browser]`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BrowserConfig {
    /// Limit pojedynczego polecenia CDP (ms).
    pub call_timeout_ms: u64,
    /// Czekanie na załadowanie strony po akcji (ms).
    pub settle_ms: u64,
    /// Najwięcej równoległych sesji.
    pub max_sessions: usize,
}

impl Default for BrowserConfig {
    fn default() -> Self {
        Self {
            call_timeout_ms: BROWSER_CALL_TIMEOUT_MS,
            settle_ms: 10_000,
            max_sessions: 4,
        }
    }
}

struct Session {
    page: page::Page,
    process: Mutex<Box<dyn ProcessGuard>>,
}

impl Drop for Session {
    fn drop(&mut self) {
        self.page.close();
        self.process
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .kill();
    }
}

/// Przeglądarka przez CDP.
pub struct CdpBrowser {
    config: BrowserConfig,
    launcher: Box<dyn Launcher>,
    sessions: Mutex<BTreeMap<u64, Arc<Session>>>,
    next: AtomicU64,
}

impl std::fmt::Debug for CdpBrowser {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CdpBrowser")
            .field("config", &self.config)
            .finish_non_exhaustive()
    }
}

impl CdpBrowser {
    /// Przeglądarka z wybranym sposobem uruchamiania.
    pub fn with_launcher(config: BrowserConfig, launcher: Box<dyn Launcher>) -> Self {
        Self {
            config,
            launcher,
            sessions: Mutex::new(BTreeMap::new()),
            next: AtomicU64::new(1),
        }
    }

    /// Przeglądarka systemowa (Windows: Edge/Chrome; inne systemy: niedostępna).
    pub fn system(config: BrowserConfig) -> Self {
        #[cfg(windows)]
        let launcher: Box<dyn Launcher> = Box::new(WinLauncher);
        #[cfg(not(windows))]
        let launcher: Box<dyn Launcher> = Box::new(NoLauncher);
        Self::with_launcher(config, launcher)
    }

    fn lock(&self) -> MutexGuard<'_, BTreeMap<u64, Arc<Session>>> {
        self.sessions.lock().unwrap_or_else(|p| p.into_inner())
    }

    fn session(&self, id: BrowserSessionId) -> Result<Arc<Session>, BrowserError> {
        let s = self
            .lock()
            .get(&id.0)
            .cloned()
            .ok_or_else(|| BrowserError::NotFound(format!("sesja przeglądarki {}", id.0)))?;
        if s.page.cdp.is_closed() {
            self.lock().remove(&id.0);
            return Err(BrowserError::Closed(
                "przeglądarka zakończyła działanie".into(),
            ));
        }
        Ok(s)
    }
}

impl BrowserPort for CdpBrowser {
    fn open(
        &self,
        spec: &BrowserSpec,
        egress: Arc<dyn EgressFilter>,
    ) -> Result<BrowserSessionId, BrowserError> {
        spec.validate()?;
        if self.lock().len() >= self.config.max_sessions {
            return Err(BrowserError::Policy(
                "za dużo otwartych przeglądarek".into(),
            ));
        }
        let launched = self.launcher.launch(spec, &chromium_args(spec))?;
        let mut process = launched.process;
        let timeout = Duration::from_millis(self.config.call_timeout_ms.max(100));
        let page =
            cdp::Cdp::start(launched.reader, launched.writer, egress, timeout).and_then(|c| {
                page::Page::setup(
                    c,
                    spec.quarantine_dir.clone(),
                    Duration::from_millis(self.config.settle_ms),
                )
            });
        let page = match page {
            Ok(p) => p,
            Err(e) => {
                process.kill();
                return Err(e);
            }
        };
        let id = self.next.fetch_add(1, Ordering::SeqCst);
        self.lock().insert(
            id,
            Arc::new(Session {
                page,
                process: Mutex::new(process),
            }),
        );
        Ok(BrowserSessionId(id))
    }

    fn navigate(&self, session: BrowserSessionId, url: &str) -> Result<PageInfo, BrowserError> {
        check_navigation_url(url)?;
        self.session(session)?.page.navigate(url)
    }

    fn snapshot(
        &self,
        session: BrowserSessionId,
        max_nodes: usize,
        max_text: usize,
    ) -> Result<PageSnapshot, BrowserError> {
        self.session(session)?.page.snapshot(max_nodes, max_text)
    }

    fn click(&self, session: BrowserSessionId, node: u32) -> Result<PageInfo, BrowserError> {
        self.session(session)?.page.click(node)
    }

    fn type_text(
        &self,
        session: BrowserSessionId,
        node: u32,
        text: &str,
        submit: bool,
    ) -> Result<PageInfo, BrowserError> {
        if text.chars().count() > MAX_TYPE_CHARS
            || text.chars().any(|c| c.is_control() && c != '\n')
        {
            return Err(BrowserError::Policy(
                "tekst za długi albo ze znakami sterującymi".into(),
            ));
        }
        self.session(session)?.page.type_text(node, text, submit)
    }

    fn screenshot(
        &self,
        session: BrowserSessionId,
        max_side: u32,
    ) -> Result<Vec<u8>, BrowserError> {
        self.session(session)?.page.screenshot(max_side)
    }

    fn close(&self, session: BrowserSessionId) -> Result<(), BrowserError> {
        self.lock()
            .remove(&session.0)
            .map(drop)
            .ok_or_else(|| BrowserError::NotFound(format!("sesja przeglądarki {}", session.0)))
    }
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
