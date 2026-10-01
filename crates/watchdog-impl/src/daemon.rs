//! Proces watchdoga (część 2): skrót `Ctrl+Shift+F12` → kill-switch, peer Brokera wołany
//! blokująco (IPC przez named pipe) na osobnym wątku, żeby limit 100 ms watchdoga działał
//! także wtedy, gdy Broker wisi — drzewa procesów (Job Objects) giną zawsze, bez czekania.

use std::sync::Arc;

use async_trait::async_trait;
use platform_contract::{HotkeyEvent, HotkeyId};
use watchdog_contract::{Clock, KillReason, KillReport, KillSwitch};

use crate::WatchdogService;

/// Najkrótszy odstęp między kolejnymi kill-switchami ze skrótu (autorepeat, drżenie klawisza).
pub const DEBOUNCE_MS: u64 = 300;

/// Obsługa zdarzeń skrótu kill-switcha.
pub struct KillSwitchDaemon {
    service: Arc<WatchdogService>,
    clock: Arc<dyn Clock>,
    hotkey: HotkeyId,
    last_kill_ms: Option<u64>,
}

impl KillSwitchDaemon {
    /// Demon dla zarejestrowanego skrótu `hotkey` (`WinHotkeys::register_kill_switch`).
    pub fn new(service: Arc<WatchdogService>, clock: Arc<dyn Clock>, hotkey: HotkeyId) -> Self {
        Self {
            service,
            clock,
            hotkey,
            last_kill_ms: None,
        }
    }

    /// Przetwarza zdarzenia skrótów; naciśnięcie kill-switcha → raport (puszczenia i inne skróty
    /// są ignorowane, powtórzenie w ciągu [`DEBOUNCE_MS`] też).
    pub async fn on_events(&mut self, events: &[HotkeyEvent]) -> Option<KillReport> {
        if !events.iter().any(|e| e.pressed && e.id == self.hotkey) {
            return None;
        }
        let now = self.clock.now_ms();
        if self
            .last_kill_ms
            .is_some_and(|t| now.saturating_sub(t) < DEBOUNCE_MS)
        {
            return None;
        }
        self.last_kill_ms = Some(now);
        Some(self.service.kill_all(KillReason::Hotkey).await)
    }
}

type PeerCall = dyn Fn(KillReason) -> Result<KillReport, String> + Send + Sync;

/// Peer kill-switcha z blokującym wywołaniem (np. `KillAll` przez named pipe do Brokera).
/// Wywołanie biegnie na osobnym wątku; future czeka na kanał, więc `tokio::time::timeout`
/// watchdoga przerywa oczekiwanie po 100 ms nawet przy zawieszonym Brokerze.
pub struct ThreadedPeer {
    call: Arc<PeerCall>,
}

impl ThreadedPeer {
    /// Peer z funkcją wywołania (błąd = raport bez unieważnionych tokenów).
    pub fn new(
        call: impl Fn(KillReason) -> Result<KillReport, String> + Send + Sync + 'static,
    ) -> Self {
        Self {
            call: Arc::new(call),
        }
    }
}

fn empty(reason: KillReason) -> KillReport {
    KillReport {
        reason,
        tokens_revoked: 0,
        jobs_killed: 0,
        jobs_failed: Vec::new(),
        audio_silenced: false,
        audited: false,
        latency_us: 0,
    }
}

#[async_trait]
impl KillSwitch for ThreadedPeer {
    async fn kill_all(&self, reason: KillReason) -> KillReport {
        let (tx, rx) = tokio::sync::oneshot::channel();
        let call = self.call.clone();
        let r = reason.clone();
        let spawned = std::thread::Builder::new()
            .name("alfa-watchdog-peer".into())
            .spawn(move || {
                let _ = tx.send(call(r));
            });
        if spawned.is_err() {
            return empty(reason);
        }
        match rx.await {
            Ok(Ok(report)) => report,
            Ok(Err(_)) | Err(_) => empty(reason),
        }
    }
}
