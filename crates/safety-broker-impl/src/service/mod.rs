//! Usługa Brokera (część 2): serwer IPC na named pipe z ACL na SID-y (bez TCP), weryfikacja
//! procesu po drugiej stronie potoku (konto, integralność, obraz, podpis) dla każdej roli,
//! Audyt w katalogu prywatnym konta usługi (kotwica obok, poza zasięgiem agentek), nadzór
//! Broker-UI w sesji użytkownika. Wszystko przez porty `platform-contract` — binarka
//! `alfa-broker` (crate `app-safety`) podstawia `platform-windows-impl`, testy — `platform-fake`.

mod bindings;
mod blocking;
mod ui;

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::Duration;

use core_log_contract::RegexRedactor;
use platform_contract::{
    CodeSignaturePort, PipeConnection, PipeSecurity, PrivateDirPort, ProcessIdentityPort,
    SecurePipePort, SessionLauncherPort, Sid, StopSignal,
};
use safety_broker_contract::ipc_blocking::UiLaunchTicket;
use serde::{Deserialize, Serialize};
use serde_json::json;
use watchdog_contract::Clock;

use crate::audit::{BrokerAuditWriter, FileAnchorStore};
use crate::engine::BrokerEngine;
use crate::ipc::{BrokerServer, IpcError};

pub use bindings::{RoleBinding, RoleBindings};
pub use blocking::BlockingIo;
pub use ui::{UiLaunchConfig, UiStep, UiSupervisor};

/// Audyt: odrzucone połączenie IPC (tożsamość procesu nie pasuje do roli, złe poświadczenie).
pub const EVENT_IPC_REJECTED: &str = "broker.ipc.rejected";

/// Konfiguracja usługi (plik JSON; ścieżka w argumencie binarki).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServiceConfig {
    /// Nazwa potoku (np. `alfa-broker`).
    pub pipe_name: String,
    /// Konto usługi Brokera (właściciel potoku i katalogu danych).
    pub broker_user: Sid,
    /// Konta klientów (interaktywny użytkownik).
    pub client_users: Vec<Sid>,
    /// Katalog prywatny: Audyt i kotwica (DACL: tylko konto usługi i SYSTEM).
    pub data_dir: PathBuf,
    /// Profil właściciela (`C:\Users\ala`) — polityka bazowa i rozwijanie zmiennych.
    pub user_profile: String,
    /// Wiązania ról z tożsamością procesów.
    pub bindings: RoleBindings,
    /// Uruchamianie Broker-UI (brak = okno uruchamia ktoś inny, np. tryb testowy).
    #[serde(default)]
    pub broker_ui: Option<UiLaunchConfig>,
    /// Tryb deweloperski (konsola, bez wymogu wysokiej integralności Broker-UI) — głośno logowany.
    #[serde(default)]
    pub dev_mode: bool,
}

impl ServiceConfig {
    /// Walidacja (nazwa potoku, wiązania ról).
    pub fn validate(&self) -> Result<(), String> {
        self.pipe_security()?;
        self.bindings.validate(self.dev_mode)
    }

    /// Zabezpieczenia potoku: pełny dostęp konto usługi, klienci — odczyt/zapis danych.
    pub fn pipe_security(&self) -> Result<PipeSecurity, String> {
        PipeSecurity::new(
            &self.pipe_name,
            self.broker_user.clone(),
            self.client_users.clone(),
        )
        .map_err(|e| e.to_string())
    }
}

/// Porty usługi.
#[derive(Clone)]
pub struct ServicePorts {
    /// Potoki z ACL.
    pub pipes: Arc<dyn SecurePipePort>,
    /// Tożsamość procesów.
    pub identity: Arc<dyn ProcessIdentityPort>,
    /// Podpisy Authenticode (dev: „niezweryfikowane”).
    pub signatures: Arc<dyn CodeSignaturePort>,
    /// Uruchamianie Broker-UI.
    pub launcher: Arc<dyn SessionLauncherPort>,
}

/// Otwiera Audyt w katalogu prywatnym (tworzy go albo wymusza chroniony DACL) z kotwicą obok.
pub fn open_audit(
    dirs: &dyn PrivateDirPort,
    data_dir: &Path,
    owner: &Sid,
    clock: Arc<dyn Clock>,
    predecessor: Option<&str>,
) -> Result<Arc<BrokerAuditWriter>, String> {
    dirs.ensure_private_dir(data_dir, owner)
        .map_err(|e| format!("katalog danych Brokera: {e}"))?;
    let anchor = Arc::new(FileAnchorStore::new(data_dir.join("audit-anchor.json")));
    let chain_id = format!("broker-{}", clock.now_ms());
    BrokerAuditWriter::open(
        data_dir.join("audit").join("broker-audit.ndjson"),
        anchor,
        Arc::new(RegexRedactor::default()),
        clock,
        &chain_id,
        predecessor,
    )
    .map(Arc::new)
    .map_err(|e| format!("Audyt: {e}"))
}

/// Usługa Brokera.
pub struct BrokerService {
    engine: Arc<BrokerEngine>,
    server: BrokerServer,
    config: ServiceConfig,
    ports: ServicePorts,
    runtime: tokio::runtime::Handle,
    stop: StopSignal,
}

impl BrokerService {
    /// Usługa nad silnikiem; niepoprawna konfiguracja → błąd (nie startuje).
    pub fn new(
        engine: Arc<BrokerEngine>,
        config: ServiceConfig,
        ports: ServicePorts,
        runtime: tokio::runtime::Handle,
        stop: StopSignal,
    ) -> Result<Self, String> {
        config.validate()?;
        Ok(Self {
            server: BrokerServer::new(engine.clone()),
            engine,
            config,
            ports,
            runtime,
            stop,
        })
    }

    /// Tworzy potok (pierwsza instancja zajęta = możliwe przejęcie nazwy → błąd) i uruchamia
    /// wątki: przyjmowanie połączeń i nadzór Broker-UI. Wraca od razu.
    pub fn start(self: &Arc<Self>) -> Result<Vec<JoinHandle<()>>, String> {
        let security = self.config.pipe_security()?;
        let mut listener = self
            .ports
            .pipes
            .listen(&security)
            .map_err(|e| format!("potok {}: {e}", security.path()))?;
        let mut threads = Vec::new();
        let me = self.clone();
        let accept = std::thread::Builder::new()
            .name("alfa-broker-accept".into())
            .spawn(move || {
                while !me.stop.is_stopped() {
                    match listener.accept() {
                        Ok(conn) => me.spawn_connection(conn),
                        Err(e) => {
                            me.log(&format!("przyjęcie połączenia: {e}"));
                            me.stop.wait(Duration::from_millis(100));
                        }
                    }
                }
            })
            .map_err(|e| e.to_string())?;
        threads.push(accept);
        if let Some(ui) = self.config.broker_ui.clone() {
            let me = self.clone();
            let t = std::thread::Builder::new()
                .name("alfa-broker-ui-supervisor".into())
                .spawn(move || me.supervise_ui(&ui))
                .map_err(|e| e.to_string())?;
            threads.push(t);
        }
        Ok(threads)
    }

    fn log(&self, message: &str) {
        eprintln!("[alfa-broker] {message}");
    }

    fn spawn_connection(self: &Arc<Self>, conn: Box<dyn PipeConnection>) {
        let me = self.clone();
        let spawned = std::thread::Builder::new()
            .name("alfa-broker-conn".into())
            .spawn(move || me.handle_connection(conn));
        if let Err(e) = spawned {
            self.log(&format!("wątek połączenia: {e}"));
        }
    }

    /// Obsługuje jedno połączenie do końca (na bieżącym wątku).
    pub fn handle_connection(&self, conn: Box<dyn PipeConnection>) {
        let pid = conn.peer_pid();
        let mut peer = match self.ports.identity.identify(pid) {
            Ok(peer) => peer,
            Err(e) => {
                self.reject(json!({ "pid": pid, "reason": format!("tożsamość nieznana: {e}") }));
                return;
            }
        };
        peer.signature = self.ports.signatures.verify(&peer.image);
        let engine = self.engine.clone();
        let bindings = &self.config.bindings;
        let peer = &peer;
        let result = self.runtime.block_on(self.server.serve_with(
            BlockingIo(conn),
            |hello| {
                bindings.authorize(hello, peer, |c| {
                    engine
                        .verify_client_credential(c)
                        .map_err(|e| e.to_string())
                })
            },
            // Odmowa trafia do Audytu przed odpowiedzią dla klienta.
            |reason| {
                self.reject(json!({
                    "pid": pid, "image": peer.image, "user": peer.user,
                    "integrity": peer.integrity, "reason": reason,
                }));
            },
        ));
        match result {
            // Odmowa jest już zapisana przez `on_reject`.
            Ok(()) | Err(IpcError::Closed | IpcError::Rejected(_)) => {}
            Err(e) => self.log(&format!("połączenie {pid}: {e}")),
        }
    }

    fn reject(&self, payload: serde_json::Value) {
        self.log(&format!("odrzucono połączenie: {payload}"));
        let _ = self.engine.audit(EVENT_IPC_REJECTED, None, payload);
    }

    fn supervise_ui(&self, config: &UiLaunchConfig) {
        let mut sup = UiSupervisor::new(config);
        let base = UiLaunchTicket {
            credential: self.engine.issue_client_credential(
                "broker-ui",
                safety_broker_contract::ipc::ClientRole::BrokerUi,
                1,
            ),
            pipe: self.config.pipe_name.clone(),
            broker_user: Some(self.config.broker_user.to_string()),
        };
        if self.config.dev_mode {
            self.log("TRYB DEWELOPERSKI: Broker-UI bez wymogu wysokiej integralności — UIPI nie chroni okna");
        }
        loop {
            let step = sup.step(
                &self.engine,
                self.ports.launcher.as_ref(),
                config,
                &base,
                self.engine.now(),
            );
            if let Some(e) = step.error {
                self.log(&e);
            }
            if self.stop.wait(Duration::from_millis(step.wait_ms)) {
                break;
            }
        }
    }
}
