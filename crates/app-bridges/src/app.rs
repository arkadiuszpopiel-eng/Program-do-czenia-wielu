//! `BridgesApp`: karty zgodności, przełącznik trasy (tylko użytkownik), przypięcie wersji
//! i zgoda na harmonogram per trasa (konfiguracja `agent_backends.*`, zmienia ją tylko
//! użytkownik), „Zaloguj w terminalu" (polecenie do skopiowania — Alfa niczego nie wykonuje
//! i nie czyta poświadczeń CLI) oraz backend mostów przebudowywany po zmianie ustawień.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

use accounts_hub_contract::{CliBridge, CliProbe, detect_cli_bridges_with};
use agent_backends_contract::{
    AgentBackend, ApprovalSink, BridgeKind, CliPin, LaunchPolicy, ScheduleConsent,
};
use agent_backends_impl::{BackendDeps, BridgeBackend, BridgeConfig, GitWorkspace};
use app_api::AppError;
use app_api::dto::{BridgeAccount, BridgeCard, BridgeLogin};
use app_api::ports::{ShellPort, default_terminal_shell};
use compliance_contract::{ChangeOrigin, Compliance, RouteId, RouteMode, RouteOrigin};
use core_bus_contract::EventBus;
use core_config_contract::{ConfigKey, ConfigLayer, ConfigStore, Origin, Scope};
use mcp_contract::BridgeMcpHost;

use crate::cards::{CardInput, bridge_from_key, bridge_key, card, login_command};

/// Jak długo ważny jest wynik wykrycia CLI (wykrycie uruchamia `--version`).
pub const DETECT_TTL: Duration = Duration::from_secs(30);
/// Najwyższa zgoda na uruchomienia z harmonogramu na dobę.
pub const MAX_SCHEDULE_PER_DAY: u32 = 24;

/// Zależności.
pub struct BridgesParts {
    /// Rejestr zgodności.
    pub compliance: Arc<dyn Compliance>,
    /// Konfiguracja (`agent_backends.*`).
    pub config: Arc<dyn ConfigStore>,
    /// Wykrywanie CLI w PATH.
    pub probe: Arc<dyn CliProbe>,
    /// Powłoka (terminal do logowania).
    pub shell: Arc<dyn ShellPort>,
    /// Kanał zatwierdzeń (Broker).
    pub sink: Arc<dyn ApprovalSink>,
    /// Serwer MCP Alfy (na żądanie).
    pub mcp: Arc<dyn BridgeMcpHost>,
    /// Pliki robocze mostów (konfiguracja MCP 0600).
    pub runtime_dir: PathBuf,
    /// Kopie robocze zadań (pod katalogiem użytkownika Alfy).
    pub worktrees: PathBuf,
    /// Katalog domowy (terminal logowania).
    pub home: PathBuf,
    /// Magistrala (`agent.bridge.*`).
    pub bus: Arc<dyn EventBus>,
    /// Backend podany z zewnątrz (testy: `agent-backends-fake`).
    pub backend: Option<Arc<dyn AgentBackend>>,
}

/// Mosty CLI w aplikacji.
pub struct BridgesApp {
    p: BridgesParts,
    current: Mutex<Option<Arc<dyn AgentBackend>>>,
    detected: Mutex<Option<(Instant, Vec<CliBridge>)>>,
    /// Ostatnie sprawdzenie konta każdego mostu (w pamięci; tylko stan, bez danych konta).
    accounts: Mutex<BTreeMap<BridgeKind, BridgeAccount>>,
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

fn key(name: &str) -> Result<ConfigKey, AppError> {
    ConfigKey::new(name).map_err(AppError::from)
}

/// Klucz konfiguracji „ostatnie logowanie wykryte w oknie Alfy” (narzędzia bez polecenia stanu).
fn login_key(kind: BridgeKind) -> String {
    format!("agent_backends.login_seen_{}", bridge_key(kind))
}

impl BridgesApp {
    /// Nowe mosty.
    pub fn new(parts: BridgesParts) -> Self {
        Self {
            p: parts,
            current: Mutex::new(None),
            detected: Mutex::new(None),
            accounts: Mutex::new(BTreeMap::new()),
        }
    }

    async fn value(&self, name: &str) -> Option<serde_json::Value> {
        let k = key(name).ok()?;
        self.p.config.get(&k, &Scope::Global).await.ok().flatten()
    }

    async fn set(&self, name: &str, value: serde_json::Value) -> Result<(), AppError> {
        self.p
            .config
            .set(
                &key(name)?,
                Some(value),
                &Scope::Global,
                &ConfigLayer::Shared,
                Origin::User,
            )
            .await
            .map_err(AppError::from)?;
        *lock(&self.current) = None;
        Ok(())
    }

    async fn pins(&self, kind: BridgeKind) -> Vec<String> {
        self.value(&format!("agent_backends.pins_{}", bridge_key(kind)))
            .await
            .and_then(|v| serde_json::from_value(v).ok())
            .unwrap_or_default()
    }

    async fn schedule(&self, kind: BridgeKind) -> u32 {
        self.value(&format!("agent_backends.schedule_{}", bridge_key(kind)))
            .await
            .and_then(|v| v.as_u64())
            .map_or(0, |n| {
                u32::try_from(n).unwrap_or(0).min(MAX_SCHEDULE_PER_DAY)
            })
    }

    /// Wykryte CLI (pamięć podręczna [`DETECT_TTL`]; wykrycie w wątku blokującym).
    pub async fn detected(&self, refresh: bool) -> Vec<CliBridge> {
        if !refresh
            && let Some((at, list)) = lock(&self.detected).as_ref()
            && at.elapsed() < DETECT_TTL
        {
            return list.clone();
        }
        let probe = self.p.probe.clone();
        let list = tokio::task::spawn_blocking(move || detect_cli_bridges_with(probe.as_ref()))
            .await
            .unwrap_or_default();
        *lock(&self.detected) = Some((Instant::now(), list.clone()));
        list
    }

    /// `bridges_list`: karty tras CLI/SDK z rejestru.
    pub async fn cards(&self) -> Vec<BridgeCard> {
        let detected = self.detected(false).await;
        let mut out = Vec::new();
        for view in self.p.compliance.views() {
            if view.origin != RouteOrigin::Registry || view.mode == RouteMode::Api {
                continue;
            }
            let kind = crate::cards::bridge_of_route(view.id.as_str());
            let route = self.p.compliance.route(&view.id);
            let found = kind.and_then(|k| detected.iter().find(|d| d.name == k.program()));
            let (pinned, schedule) = match kind {
                Some(k) => (self.pins(k).await, self.schedule(k).await),
                None => (Vec::new(), 0),
            };
            let mut c = card(CardInput {
                view: &view,
                route: route.as_ref(),
                detected: found,
                pinned,
                schedule,
            });
            c.account = kind.and_then(|k| lock(&self.accounts).get(&k).cloned());
            out.push(c);
        }
        out
    }

    /// `bridges_check`: świeże wykrycie i stan logowania mostu (oficjalne polecenie stanu
    /// narzędzia — bez czytania danych logowania); wynik zostaje na karcie.
    pub async fn check(&self, bridge: &str) -> Result<BridgeCard, AppError> {
        let kind = bridge_from_key(bridge)
            .ok_or_else(|| AppError::invalid(format!("Nieznany most „{bridge}”.")))?;
        let found = self
            .detected(true)
            .await
            .into_iter()
            .find(|d| d.name == kind.program());
        let account = match (&found, crate::login::status_args(kind)) {
            (None, _) => crate::login::not_installed(kind),
            (Some(_), None) => match self.value(&login_key(kind)).await {
                // „plan|dzień” z ostatniego logowania w oknie Alfy (narzędzie bez polecenia stanu).
                Some(serde_json::Value::String(saved)) => {
                    let (plan, day) = saved.split_once('|').unwrap_or(("", saved.as_str()));
                    let plan = Some(plan.to_owned()).filter(|p| !p.is_empty());
                    crate::login::seen(kind, plan, day)
                }
                _ => crate::login::check(kind, None),
            },
            (Some(d), Some(args)) => {
                let probe = self.p.probe.clone();
                let path = d.path.clone();
                let out = tokio::task::spawn_blocking(move || probe.run_output(&path, args))
                    .await
                    .ok()
                    .flatten();
                crate::login::check(kind, out.as_ref())
            }
        };
        lock(&self.accounts).insert(kind, account);
        self.cards()
            .await
            .into_iter()
            .find(|c| c.bridge.as_deref() == Some(bridge))
            .ok_or_else(|| AppError::not_found(format!("Brak karty mostu „{bridge}”.")))
    }

    /// `bridges_mark_login`: okno logowania wykryło zalogowanie narzędzia bez polecenia stanu
    /// (Antigravity: konto i plan w nagłówku). Zapamiętuje tylko plan i dzień — bez adresu e-mail.
    pub async fn mark_login(
        &self,
        bridge: &str,
        plan: Option<String>,
    ) -> Result<BridgeCard, AppError> {
        let kind = Self::kind(bridge)?;
        if crate::login::status_args(kind).is_some() {
            // Narzędzie ma własne polecenie stanu — ono jest źródłem prawdy.
            return self.check(bridge).await;
        }
        let plan = plan
            .map(|p| crate::login::tidy(&p))
            .filter(|p| !p.trim().is_empty())
            .unwrap_or_default();
        let day = chrono::Utc::now().format("%Y-%m-%d").to_string();
        self.set(&login_key(kind), format!("{plan}|{day}").into())
            .await?;
        self.check(bridge).await
    }

    async fn card_for(&self, route: &str) -> Result<BridgeCard, AppError> {
        self.cards()
            .await
            .into_iter()
            .find(|c| c.route_id == route)
            .ok_or_else(|| AppError::not_found(format!("Nieznana trasa „{route}”.")))
    }

    fn kind(bridge: &str) -> Result<BridgeKind, AppError> {
        bridge_from_key(bridge)
            .ok_or_else(|| AppError::invalid(format!("Nieznany most „{bridge}”.")))
    }

    /// `bridges_set_enabled`: wyłącznik trasy (tylko użytkownik; zabronionej nie da się włączyć).
    pub async fn set_enabled(&self, route: &str, enabled: bool) -> Result<BridgeCard, AppError> {
        let id = RouteId::new(route).ok_or_else(|| AppError::invalid("Nieprawidłowa trasa."))?;
        self.p
            .compliance
            .set_enabled(&id, enabled, ChangeOrigin::User)
            .await
            .map_err(|e| AppError::forbidden(format!("Zgodność: {e}")))?;
        self.card_for(route).await
    }

    /// `bridges_set_schedule`: jawna zgoda na harmonogram (0 = brak; najwyżej 24/dobę).
    pub async fn set_schedule(&self, bridge: &str, per_day: u32) -> Result<BridgeCard, AppError> {
        let kind = Self::kind(bridge)?;
        let n = per_day.min(MAX_SCHEDULE_PER_DAY);
        self.set(
            &format!("agent_backends.schedule_{}", bridge_key(kind)),
            n.into(),
        )
        .await?;
        self.card_for(kind.route_id_str()).await
    }

    /// `bridges_pin`: przypina wykrytą wersję CLI (`None` — odpina; bez przypięcia most odmawia).
    pub async fn pin(&self, bridge: &str, version: Option<String>) -> Result<BridgeCard, AppError> {
        let kind = Self::kind(bridge)?;
        let pins: Vec<String> = match version.map(|v| v.trim().to_owned()) {
            Some(v) if !v.is_empty() => {
                let detected = self.detected(true).await;
                let found = detected
                    .iter()
                    .find(|d| d.name == kind.program())
                    .and_then(|d| d.version.clone());
                if found.as_deref() != Some(v.as_str()) {
                    return Err(AppError::invalid(
                        "Przypiąć można tylko wersję wykrytą na tym komputerze.",
                    ));
                }
                vec![v]
            }
            _ => Vec::new(),
        };
        self.set(
            &format!("agent_backends.pins_{}", bridge_key(kind)),
            serde_json::json!(pins),
        )
        .await?;
        self.card_for(kind.route_id_str()).await
    }

    /// `bridges_open_login`: terminal w katalogu domowym; polecenie wpisuje użytkownik.
    pub fn open_login(&self, bridge: &str) -> Result<BridgeLogin, AppError> {
        let kind = Self::kind(bridge)?;
        let opened = self
            .p
            .shell
            .open_terminal(&self.p.home, default_terminal_shell());
        if let Err(e) = &opened {
            tracing::info!(error = %e.message, "terminal logowania niedostępny — polecenie do skopiowania");
        }
        Ok(BridgeLogin {
            command: login_command(kind).into(),
            cwd: self.p.home.to_string_lossy().into_owned(),
            opened: opened.is_ok(),
        })
    }

    /// Backend mostów (przebudowany po zmianie przypięć/zgód).
    pub async fn backend(&self) -> Arc<dyn AgentBackend> {
        if let Some(b) = &self.p.backend {
            return b.clone();
        }
        if let Some(b) = lock(&self.current).clone() {
            return b;
        }
        let detected = self.detected(false).await;
        let mut config = BridgeConfig::new(self.p.runtime_dir.clone());
        let mut scheduled = BTreeMap::new();
        for kind in BridgeKind::ALL {
            let program = detected
                .iter()
                .find(|d| d.name == kind.program())
                .map_or_else(|| PathBuf::from(kind.program()), |d| d.path.clone());
            let pin = CliPin {
                versions: self.pins(kind).await,
                sha256: None,
            };
            config = config.with_bridge(kind, program, pin);
            let n = self.schedule(kind).await;
            if n > 0 {
                scheduled.insert(
                    kind.route_id_str().to_owned(),
                    ScheduleConsent { max_per_day: n },
                );
            }
        }
        config.launch = LaunchPolicy { scheduled };
        let backend: Arc<dyn AgentBackend> = Arc::new(
            BridgeBackend::new(
                config,
                BackendDeps {
                    compliance: self.p.compliance.clone(),
                    workspace: Arc::new(GitWorkspace::new(self.p.worktrees.clone())),
                    mcp: self.p.mcp.clone(),
                    sink: self.p.sink.clone(),
                },
            )
            .with_bus(self.p.bus.clone()),
        );
        *lock(&self.current) = Some(backend.clone());
        backend
    }
}
