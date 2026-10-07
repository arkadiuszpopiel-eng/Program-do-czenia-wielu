//! Port do rdzenia aplikacji (`app-core` implementuje): sesja zadań w tle, katalog roboczy
//! i prywatność sesji, ustawienia agentek, zapis Replay/Osi czasu przebiegów zadań.

use std::sync::Arc;

use app_agents::{AgentSettings, AgentTools, Projection, TicketLog};
use app_api::dto::AgentRun;
use app_api::ports::BrainPort;
use async_trait::async_trait;
use core_bus_contract::{EventBus, SessionId};
use personas_contract::Personas;
use sessions_contract::PrivacyTag;

/// Rdzeń widziany przez wykonawczynię zadań.
#[async_trait]
pub trait TaskHost: Send + Sync {
    /// Sesja dla zadań bez sesji (wyzwalacze czasowe, usługi): „Zadania w tle", tworzona
    /// leniwie — Replay przebiegu ma gdzie trafić.
    async fn background_session(&self) -> Result<SessionId, String>;
    /// Katalog roboczy sesji (`None` — agentka bez narzędzi plików i powłoki).
    fn workdir(&self, session: &SessionId) -> Option<String>;
    /// Tag prywatności sesji (nieznana → prywatna).
    fn privacy(&self, session: &SessionId) -> PrivacyTag;
    /// Ustawienia agentek (budżety, limit czekania na zatwierdzenie).
    async fn agent_settings(&self) -> AgentSettings;
    /// Kurs USD→PLN × 10⁴.
    fn usd_pln_e4(&self) -> u64;
    /// Czy działa okno Brokera (Broker-UI).
    fn broker_window(&self) -> bool;
    /// Zapis i emisja projekcji przebiegu (Replay, Oś czasu, zdarzenia UI).
    fn project(&self, session: &SessionId, run: &AgentRun, projection: Projection);
    /// Przed startem przebiegu: skażenie sesji z katalogu → Broker (W3-04; trwałe po restarcie).
    /// Błąd = przebieg nie startuje (fail-closed). Domyślnie nic (host bez Brokera).
    async fn sync_taint(&self, _session: &SessionId) -> Result<(), String> {
        Ok(())
    }
}

/// Narzędzia agentek (z Brokerem i dziennikiem cofania).
#[derive(Clone)]
pub struct AgentKit {
    /// Narzędzia (fs, shell, schowek, pamięć).
    pub tools: Arc<AgentTools>,
    /// Broker z rejestrem kart zatwierdzeń.
    pub tickets: Arc<TicketLog>,
    /// Start v1: zasoby wyłączne, autonomia, obsada (delegacja, Krytyczka), umiejętności.
    pub launch: app_agents::Launch,
}

/// Zależności wykonawczyni zadań.
#[derive(Clone)]
pub struct ExecDeps {
    /// Rdzeń.
    pub host: Arc<dyn TaskHost>,
    /// Wybór modelu (Router).
    pub brain: Arc<dyn BrainPort>,
    /// Obsada i persony.
    pub personas: Arc<dyn Personas>,
    /// Narzędzia agentek (`None` — Broker niepodłączony: zadania agentek kończą się błędem).
    pub kit: Option<AgentKit>,
    /// Mosty CLI (`None` — moduł niepodłączony).
    pub bridges: Option<Arc<dyn agent_backends_contract::AgentBackend>>,
    /// Rejestr przebiegów mostów (prośby o uprawnienia → Replay).
    pub runs: Arc<crate::sink::BridgeRuns>,
    /// Magistrala (`agent.*`).
    pub bus: Arc<dyn EventBus>,
}
