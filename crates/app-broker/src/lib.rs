//! Broker poza procesem aplikacji (ADR 0003, PLAN §8.1–8.2, §8.6; SPEC-i `safety-broker`,
//! `broker-ui`, `watchdog`) — część korzenia kompozycji `app-*`.
//!
//! - [`KernelBroker`]: to, czego `app-core` używa zamiast silnika w procesie — `Broker` dla
//!   narzędzi agentek, kill-switch, rejestr Job Objects narzędzi, okno zatwierdzeń i zdrowie.
//! - [`RemoteKernel`]: Broker poza procesem (usługa `AlfaBroker` albo tryb przenośny
//!   `alfa-broker --console`) — wybór w `AppOptions::kernel`; [`RemoteKernel::unavailable`] —
//!   bezpieczny stan buildu produkcyjnego bez izolowanego Brokera (nic, co wymaga zgody).
//! - [`kernel::KernelProcesses`]: start procesów Jądra przez powłokę (tryb, `alfa-watchdog`,
//!   nadzór łącza); [`inproc`]: Broker w procesie (tryb deweloperski, Linux/CI).
//! - [`link::BrokerLink`]: klient IPC roli `Core` (sprawdzenie serwera potoku, limity czasu,
//!   fail-closed po zerwaniu).

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

pub mod children;
pub mod inproc;
pub mod kernel;
pub mod link;
pub mod mode;
pub mod notice;
mod remote;
mod status;
pub mod supervise;
mod window;

use std::sync::Arc;

use app_api::dto::BrokerMode;
use app_api::ports::ApprovalWindow;
use core_bus_contract::EventBus;
use core_registry_contract::HealthStatus;
use platform_contract::ProcessPort;
use safety_broker_contract::Broker;
use safety_broker_impl::BrokerEngine;
use watchdog_contract::{JobRegistry, KillSwitch};

pub use remote::{KILL_TIMEOUT, RemoteBroker, RemoteKill};
pub use status::{KernelStatus, LinkState};
pub use window::RemoteWindow;

use crate::link::{BrokerLink, LinkConfig};

type Health = Arc<dyn Fn() -> HealthStatus + Send + Sync>;

/// Broker widziany przez `app-core` (silnik w procesie albo Broker poza procesem).
#[derive(Clone)]
pub struct KernelBroker {
    /// `Broker` dla narzędzi agentek (decyzje, tokeny, prośby, autonomia).
    pub broker: Arc<dyn Broker>,
    /// Kill-switch (drzewa narzędzi, cisza audio, tokeny w Brokerze).
    pub kill: Arc<dyn KillSwitch>,
    /// Rejestr Job Objects narzędzi (zabijanych przez `kill`).
    pub jobs: Arc<dyn JobRegistry>,
    /// Okno zatwierdzeń i stan łącza (`None` — Broker w procesie: okno z `AppOptions`).
    pub window: Option<Arc<dyn ApprovalWindow>>,
    health: Health,
}

impl std::fmt::Debug for KernelBroker {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("KernelBroker")
            .field("remote", &self.window.is_some())
            .field("health", &self.health())
            .finish_non_exhaustive()
    }
}

impl KernelBroker {
    /// Silnik w procesie (tryb deweloperski).
    pub fn in_process(engine: Arc<BrokerEngine>) -> Self {
        Self {
            broker: engine.clone(),
            kill: engine.clone(),
            jobs: engine,
            window: None,
            health: Arc::new(|| HealthStatus::Healthy),
        }
    }

    /// Zdrowie modułu `safety-broker` w rejestrze.
    pub fn health(&self) -> HealthStatus {
        (self.health)()
    }

    /// Sonda zdrowia dla gniazda rejestru.
    pub fn health_probe(&self) -> Arc<dyn Fn() -> HealthStatus + Send + Sync> {
        self.health.clone()
    }
}

/// Broker poza procesem — wybór w `AppOptions::kernel` (łącze tworzy powłoka).
#[derive(Clone, Debug)]
pub struct RemoteKernel {
    link: Arc<BrokerLink>,
}

impl RemoteKernel {
    /// Broker nad łączem (procesy i nadzór — [`kernel::KernelProcesses`]).
    pub fn new(link: Arc<BrokerLink>) -> Self {
        Self { link }
    }

    /// Bezpieczny stan bez izolowanego Brokera (build produkcyjny bez usługi i bez binarek
    /// trybu przenośnego): łącze nigdy się nie łączy, każda decyzja to odmowa, UI pokazuje „brak”.
    pub fn unavailable(reason: &str) -> Self {
        let status = Arc::new(KernelStatus::new(BrokerMode::Unavailable, false));
        status.set_link(LinkState::Lost(reason.to_owned()));
        let refuse = Arc::new(Refuse(reason.to_owned()));
        let link = BrokerLink::new(
            refuse.clone(),
            refuse,
            LinkConfig::new("alfa-broker-unavailable"),
            status,
        );
        Self { link }
    }

    /// Stan wspólny (UI, zdrowie).
    pub fn status(&self) -> Arc<KernelStatus> {
        self.link.status().clone()
    }

    /// Łącze IPC.
    pub fn link(&self) -> &Arc<BrokerLink> {
        &self.link
    }

    /// Składa [`KernelBroker`] z portem procesów narzędzi (ten sam, który je uruchamia — uchwyty
    /// Job Objects należą do procesu aplikacji) i magistralą (cisza audio).
    pub fn bind(
        &self,
        processes: Arc<dyn ProcessPort>,
        bus: Option<Arc<dyn EventBus>>,
    ) -> KernelBroker {
        let kill = Arc::new(RemoteKill::new(self.link.clone(), processes, bus));
        let status = self.status();
        KernelBroker {
            broker: Arc::new(RemoteBroker::new(self.link.clone())),
            kill: kill.clone(),
            jobs: kill,
            window: Some(Arc::new(RemoteWindow::new(status.clone()))),
            health: Arc::new(move || status.health()),
        }
    }
}

/// Porty bez Brokera: każde połączenie odmówione (stan „brak”).
struct Refuse(String);

impl platform_contract::SecurePipePort for Refuse {
    fn listen(
        &self,
        _security: &platform_contract::PipeSecurity,
    ) -> Result<Box<dyn platform_contract::PipeListener>, platform_contract::PlatformError> {
        Err(platform_contract::PlatformError::Unsupported(
            self.0.clone(),
        ))
    }

    fn connect(
        &self,
        _name: &str,
        _timeout_ms: u32,
    ) -> Result<Box<dyn platform_contract::PipeConnection>, platform_contract::PlatformError> {
        Err(platform_contract::PlatformError::Unsupported(
            self.0.clone(),
        ))
    }
}

impl platform_contract::ProcessIdentityPort for Refuse {
    fn identify(
        &self,
        _pid: u32,
    ) -> Result<platform_contract::PeerIdentity, platform_contract::PlatformError> {
        Err(platform_contract::PlatformError::Unsupported(
            self.0.clone(),
        ))
    }

    fn current_user(&self) -> Result<platform_contract::Sid, platform_contract::PlatformError> {
        Err(platform_contract::PlatformError::Unsupported(
            self.0.clone(),
        ))
    }
}
