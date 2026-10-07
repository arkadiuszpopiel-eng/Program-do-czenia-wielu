//! Współdzielone testy kontraktowe F7 ([`MemoryService`]): warstwy i zakresy, uprawnienia agentek,
//! prywatność, proweniencja (ACCEPTANCE F7-04), wersje i sprzeczności, Inspektor, kaskada
//! `forget` (F7-03), dziennik z cofaniem, eksport/import, testy szpiegowskie (F7-01).
//!
//! Fabryka dostaje porty ([`EnginePorts`]) z zegarem wirtualnym, prywatnością (sesja `P` jest
//! prywatna) i rejestratorem zdarzeń — `-impl` i `-fake` budują z nich silnik nad swoim magazynem.

mod access;
mod changes;
mod forget;
mod inspector;
mod spy;
mod versions;

use std::ops::Deref;
use std::sync::Arc;

use core_bus_contract::{AgentId, SessionId};

use crate::access::{Accessor, AgentAccess, ScopeGrant};
use crate::engine::EnginePorts;
use crate::ports::{PrivateSessions, RecordingEvents, SeqIds, VirtualClock};
use crate::rerank::HeuristicReranker;
use crate::service::{MemoryService, RecallRequest};
use crate::types::{Layer, MemoryEntry, MemoryScope, NewMemory, Provenance, RememberMode};

/// Kontekst przypadku: zegar wirtualny, prywatność (sesja `P` prywatna), zdarzenia.
pub struct Ctx {
    /// Zegar (przesuwany w testach TTL i retencji).
    pub clock: Arc<VirtualClock>,
    /// Prywatność sesji.
    pub privacy: Arc<PrivateSessions>,
    /// Zdarzenia.
    pub events: Arc<RecordingEvents>,
}

impl Ctx {
    /// Nowy kontekst i porty silnika.
    pub fn new() -> (Self, EnginePorts) {
        let ctx = Self {
            clock: Arc::new(VirtualClock::new()),
            privacy: Arc::new(PrivateSessions::new()),
            events: Arc::new(RecordingEvents::new()),
        };
        ctx.privacy.mark_private(SessionId::new("P"));
        let ports = EnginePorts {
            clock: ctx.clock.clone(),
            ids: Arc::new(SeqIds::new()),
            privacy: ctx.privacy.clone(),
            reranker: Arc::new(HeuristicReranker),
            events: ctx.events.clone(),
        };
        (ctx, ports)
    }
}

pub(crate) fn ok<T, E: std::fmt::Display>(r: Result<T, E>) -> T {
    r.unwrap_or_else(|e| panic!("nieoczekiwany błąd: {e}"))
}

pub(crate) fn sess(id: &str) -> MemoryScope {
    MemoryScope::Session(SessionId::new(id))
}

pub(crate) fn agent_scope(id: &str) -> MemoryScope {
    MemoryScope::Agent(AgentId::new(id))
}

pub(crate) fn owner() -> Accessor {
    Accessor::Owner
}

/// Agentka z dostępem tylko do własnej sesji.
pub(crate) fn agent(name: &str, session: &str) -> Accessor {
    Accessor::Agent(AgentAccess::session_only(
        AgentId::new(name),
        SessionId::new(session),
    ))
}

/// Agentka z przyznaniami odczytu/zapisu.
pub(crate) fn agent_with(
    name: &str,
    session: &str,
    project: Option<&str>,
    read: &[ScopeGrant],
    write: &[ScopeGrant],
) -> Accessor {
    Accessor::Agent(AgentAccess {
        agent: AgentId::new(name),
        session: SessionId::new(session),
        project: project.map(str::to_owned),
        read: read.to_vec(),
        write: write.to_vec(),
    })
}

/// Fakt użytkownika w zakresie.
pub(crate) fn fact(scope: MemoryScope, text: &str) -> NewMemory {
    NewMemory::new(scope, Layer::Semantic, text, Provenance::User)
}

/// Treść niezaufana w zakresie.
pub(crate) fn untrusted(scope: MemoryScope, text: &str, source: &str) -> NewMemory {
    NewMemory::new(
        scope,
        Layer::Semantic,
        text,
        Provenance::UntrustedContent {
            source: source.into(),
        },
    )
}

pub(crate) fn put(m: &dyn MemoryService, new: NewMemory) -> MemoryEntry {
    ok(m.remember_as(&owner(), new, RememberMode::Explicit))
}

/// Teksty wyników `recall`.
pub(crate) fn recall_texts(
    m: &dyn MemoryService,
    who: &Accessor,
    scopes: Vec<MemoryScope>,
    query: &str,
    k: usize,
) -> Vec<String> {
    ok(m.recall_as(who, &RecallRequest::new(scopes, query, k)))
        .into_iter()
        .map(|r| r.entry.text)
        .collect()
}

/// Deterministyczny generator (xorshift64*).
pub(crate) struct Rng(u64);

impl Rng {
    pub(crate) fn new(seed: u64) -> Self {
        Self(seed.max(1))
    }

    pub(crate) fn below(&mut self, n: usize) -> usize {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        let x = self.0.wrapping_mul(0x2545_f491_4f6c_dd1d);
        usize::try_from(x % (n.max(1) as u64)).unwrap_or(0)
    }
}

/// Przypadek testowy.
pub type Case = fn(&dyn MemoryService, &Ctx);

/// Wszystkie przypadki (nazwa, funkcja).
pub fn cases() -> Vec<(&'static str, Case)> {
    vec![
        ("layers_and_scopes", access::layers_and_scopes),
        ("agent_access_is_scoped", access::agent_access_is_scoped),
        (
            "private_session_never_feeds_broader",
            access::private_session_never_feeds_broader,
        ),
        ("untrusted_never_promotes", access::untrusted_never_promotes),
        (
            "versions_and_contradictions",
            versions::versions_and_contradictions,
        ),
        ("pin_and_working_set", versions::pin_and_working_set),
        ("ttl_and_pending", versions::ttl_and_pending),
        ("inspector_filters", inspector::inspector_filters),
        ("explain_why", inspector::explain_why),
        (
            "export_import_round_trip",
            inspector::export_import_round_trip,
        ),
        ("forget_entry_family", forget::forget_entry_family),
        ("forget_session_cascade", forget::forget_session_cascade),
        ("forget_source_turn_scope", forget::forget_source_turn_scope),
        ("forget_fifty_verified", forget::forget_fifty_verified),
        (
            "forget_same_id_in_other_scope",
            forget::forget_same_id_in_other_scope,
        ),
        ("changes_and_undo", changes::changes_and_undo),
        ("changes_are_atomic", changes::changes_are_atomic),
        ("resolve_and_undo", changes::resolve_and_undo),
        ("spy_three_sessions", spy::spy_three_sessions),
        ("events_without_content", spy::events_without_content),
    ]
}

/// Uruchamia cały zestaw; `factory` buduje świeżą pamięć z podanych portów.
pub fn run_all<H, M>(factory: impl Fn(EnginePorts) -> H)
where
    H: Deref<Target = M>,
    M: MemoryService,
{
    for (name, case) in cases() {
        let (ctx, ports) = Ctx::new();
        let harness = factory(ports);
        eprintln!("przypadek F7: {name}");
        case(&*harness, &ctx);
    }
}
