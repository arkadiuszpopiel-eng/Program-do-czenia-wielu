//! Nadzór łącza z Brokerem (osobny wątek): heartbeat (`Metrics`) przy połączeniu, ponowne
//! łączenie po zerwaniu, w trybie przenośnym — ponowne uruchomienie `alfa-broker --console`
//! z rosnącą przerwą (1 s → 30 s). Każde nowe uruchomienie to nowy klucz Brokera: tokeny
//! i prośby poprzedniego wygasają (bezpiecznie — agentki proszą od nowa).

use std::sync::Arc;
use std::time::{Duration, Instant};

use safety_broker_contract::ipc::Request;

use crate::children::{ChildProc, ChildSpec, LastLine, Spawner, pump_lines};
use crate::link::{BrokerLink, LinkError, ServerCheck};
use crate::status::LinkState;

/// Odstępy nadzoru.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Timing {
    /// Ile przy starcie aplikacji czekać na pierwsze połączenie.
    pub first_connect: Duration,
    /// Odstęp prób ponownego połączenia.
    pub retry: Duration,
    /// Odstęp heartbeatu przy połączeniu.
    pub heartbeat: Duration,
    /// Ile czekać na gotowość watchdoga (rejestracja skrótu).
    pub watchdog_ready: Duration,
    /// Początkowa przerwa przed ponownym uruchomieniem Brokera (rośnie do 30 s).
    pub relaunch: Duration,
}

impl Default for Timing {
    fn default() -> Self {
        Self {
            first_connect: Duration::from_secs(5),
            retry: Duration::from_millis(250),
            heartbeat: Duration::from_secs(1),
            watchdog_ready: Duration::from_secs(2),
            relaunch: Duration::from_secs(1),
        }
    }
}

/// Maksymalna przerwa między uruchomieniami Brokera.
const MAX_RELAUNCH: Duration = Duration::from_secs(30);
/// Po takim czasie połączenia przerwa wraca do początkowej.
const HEALTHY_AFTER: Duration = Duration::from_secs(30);

/// Do czego się łączyć.
pub enum Target {
    /// Usługa (sprawdzenie sesji 0 i konta).
    Service(ServerCheck),
    /// Konfiguracja usługi uszkodzona — bezpieczny stan bez prób.
    Broken(String),
    /// Tryb przenośny: proces potomny.
    Portable {
        /// `alfa-broker --console --lifeline`.
        spec: ChildSpec,
        /// Uruchamianie.
        spawner: Arc<dyn Spawner>,
    },
}

/// Stan nadzoru (krok testowalny bez wątku).
pub struct Supervisor {
    link: Arc<BrokerLink>,
    target: Target,
    timing: Timing,
    child: Option<Box<dyn ChildProc>>,
    log: LastLine,
    backoff: Duration,
    next_launch: Option<Instant>,
    connected_at: Option<Instant>,
    last_beat: Option<Instant>,
    launches: u32,
}

impl Supervisor {
    /// Nadzór łącza.
    pub fn new(link: Arc<BrokerLink>, target: Target, timing: Timing) -> Self {
        Self {
            link,
            target,
            backoff: timing.relaunch,
            timing,
            child: None,
            log: LastLine::default(),
            next_launch: None,
            connected_at: None,
            last_beat: None,
            launches: 0,
        }
    }

    /// PID procesu Brokera (tryb przenośny).
    pub fn broker_pid(&self) -> Option<u32> {
        self.child.as_ref().map(|c| c.pid())
    }

    /// Ile razy uruchomiono Brokera (tryb przenośny).
    pub fn launches(&self) -> u32 {
        self.launches
    }

    fn fail(&self, why: String, quiet: bool) {
        // W trakcie startu (potok jeszcze nie istnieje) stan zostaje „łączenie”.
        if !quiet {
            self.link.status().set_link(LinkState::Lost(why));
        }
    }

    /// Jeden krok; zwraca, ile czekać do następnego. `quiet` — start aplikacji (bez stanu „zerwane”).
    pub fn step(&mut self, quiet: bool) -> Duration {
        if self.link.status().connected() {
            return self.watch_connected();
        }
        self.connected_at = None;
        let check = match &self.target {
            Target::Broken(why) => {
                self.fail(format!("usługa Brokera: {why}"), false);
                return self.timing.heartbeat.max(self.timing.retry);
            }
            Target::Service(check) => check.clone(),
            Target::Portable { .. } => match self.ensure_child() {
                Ok(pid) => ServerCheck::Pid(pid),
                Err(why) => {
                    self.fail(why, quiet);
                    return self.timing.retry;
                }
            },
        };
        match self.link.connect(&check) {
            Ok(()) => {
                self.connected_at = Some(Instant::now());
                self.last_beat = Some(Instant::now());
                self.timing.heartbeat
            }
            Err(e) => {
                let quiet = quiet && matches!(e, LinkError::Lost(_));
                self.fail(e.to_string(), quiet);
                self.timing.retry
            }
        }
    }

    fn watch_connected(&mut self) -> Duration {
        if let Some(child) = self.child.as_mut()
            && !child.running()
        {
            let why = self.log.get().map_or_else(
                || "proces Brokera zakończył działanie".to_owned(),
                |l| format!("proces Brokera zakończył działanie: {l}"),
            );
            self.link.disconnect(&why);
            return Duration::ZERO;
        }
        if self
            .connected_at
            .is_some_and(|t| t.elapsed() >= HEALTHY_AFTER)
        {
            self.backoff = self.timing.relaunch;
        }
        let due = self
            .last_beat
            .is_none_or(|t| t.elapsed() >= self.timing.heartbeat);
        if due {
            self.last_beat = Some(Instant::now());
            // Błąd strumienia albo limit czasu oznacza łącze jako zerwane w samym łączu.
            if let Err(e) = self.link.call(Request::Metrics) {
                tracing::warn!(error = %e, "heartbeat Brokera nieudany");
            }
        }
        self.timing.heartbeat
    }

    /// Tryb przenośny: działający proces Brokera (uruchamia ponownie po przerwie).
    fn ensure_child(&mut self) -> Result<u32, String> {
        if let Some(child) = self.child.as_mut() {
            if child.running() {
                return Ok(child.pid());
            }
            let why = self.log.get().unwrap_or_else(|| "bez komunikatu".into());
            self.child = None;
            self.next_launch = Some(Instant::now() + self.backoff);
            self.backoff = (self.backoff * 2).min(MAX_RELAUNCH);
            return Err(format!("proces Brokera zakończył działanie ({why})"));
        }
        if self.next_launch.is_some_and(|t| Instant::now() < t) {
            return Err("Broker zostanie uruchomiony ponownie za chwilę".into());
        }
        let Target::Portable { spec, spawner } = &self.target else {
            return Err("brak procesu Brokera do uruchomienia".into());
        };
        let mut child = spawner
            .spawn(spec)
            .map_err(|e| format!("uruchomienie Brokera nieudane: {e}"))?;
        self.log = LastLine::default();
        if let Some(err) = child.take_stderr() {
            pump_lines("broker", err, self.log.clone(), |line| {
                tracing::info!(target: "alfa-broker", "{line}");
            });
        }
        if let Some(out) = child.take_stdout() {
            pump_lines("broker-out", out, LastLine::default(), |line| {
                tracing::info!(target: "alfa-broker", "{line}");
            });
        }
        let pid = child.pid();
        self.launches = self.launches.saturating_add(1);
        tracing::warn!(
            pid,
            "tryb przenośny: uruchomiono alfa-broker --console (słabsza izolacja)"
        );
        self.child = Some(child);
        Ok(pid)
    }
}

impl Drop for Supervisor {
    fn drop(&mut self) {
        // Proces Brokera trybu przenośnego kończy się razem z nadzorem (zamknięcie aplikacji).
        if let Some(child) = self.child.as_mut() {
            child.kill();
        }
    }
}
