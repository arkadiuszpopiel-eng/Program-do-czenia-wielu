//! Wspólne narzędzia testów Strażniczki: silnik atrapy pamięci + atrapy portów.

#![allow(dead_code, clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use memory_consolidation_contract::{ConsolidationConfig, Guardian, GuardianPorts, HostConditions};
use memory_consolidation_fake::{FakeHost, FixedBudget, ScriptedConsolidator};
use memory_contract::{
    Accessor, EnginePorts, InspectorQuery, Layer, MemoryScope, MemoryService, NewMemory,
    PrivateSessions, Provenance, RecordingEvents, RememberMode, SessionId, VirtualClock,
};

pub struct World {
    pub memory: Arc<dyn MemoryService>,
    pub clock: Arc<VirtualClock>,
    pub model: Arc<ScriptedConsolidator>,
    pub budget: Arc<FixedBudget>,
    pub host: Arc<FakeHost>,
    pub events: Arc<RecordingEvents>,
    pub privacy: Arc<PrivateSessions>,
}

pub fn world(model: ScriptedConsolidator) -> World {
    let clock = Arc::new(VirtualClock::new());
    let privacy = Arc::new(PrivateSessions::new());
    privacy.mark_private(SessionId::new("P"));
    let ports = EnginePorts {
        clock: clock.clone(),
        privacy: privacy.clone(),
        ..EnginePorts::deterministic()
    };
    World {
        memory: Arc::new(memory_fake::service_with(ports)),
        clock,
        model: Arc::new(model),
        budget: Arc::new(FixedBudget::allow()),
        host: Arc::new(FakeHost::idle_night()),
        events: Arc::new(RecordingEvents::new()),
        privacy,
    }
}

pub fn guardian(w: &World, config: ConsolidationConfig, host: Arc<dyn HostConditions>) -> Guardian {
    let ports = GuardianPorts {
        memory: w.memory.clone(),
        consolidator: Some(w.model.clone()),
        budget: w.budget.clone(),
        host,
        privacy: w.privacy.clone(),
        events: w.events.clone(),
        clock: w.clock.clone(),
    };
    Guardian::new(ports, config)
}

pub fn put(
    m: &dyn MemoryService,
    scope: MemoryScope,
    layer: Layer,
    text: &str,
) -> memory_contract::MemoryEntry {
    m.remember_as(
        &Accessor::Owner,
        NewMemory::new(scope, layer, text, Provenance::User),
        RememberMode::Explicit,
    )
    .unwrap()
}

pub fn sess(s: &str) -> MemoryScope {
    MemoryScope::Session(SessionId::new(s))
}

pub fn all(m: &dyn MemoryService, scope: &MemoryScope) -> Vec<memory_contract::MemoryEntry> {
    m.inspect(
        &Accessor::Owner,
        &InspectorQuery {
            scopes: vec![scope.clone()],
            limit: 500,
            ..Default::default()
        },
    )
    .unwrap()
    .items
    .into_iter()
    .map(|i| i.entry)
    .collect()
}
