//! Wspólny kontrakt narzędzi agentek `tools-*` (docs/modules/tools-common/SPEC.md, PLAN §7.2,
//! §8.7, §9.1). Odpowiedź na otwarte pytanie SPEC `tools-fs`: jeden format manifestu dla
//! wszystkich narzędzi (i w przyszłości MCP).
//!
//! - [`ToolManifest`]: nazwa i opis dla modelu, JSON Schema wejścia/wyjścia, `reversible:
//!   yes|scoped|no`, wymagane zdolności, grupy ról, źródło niezaufanej treści;
//! - [`Tool`]: wywołanie z argumentami JSON w [`ToolCtx`] → [`ToolOutcome`] (status, tekst dla
//!   modelu, dane, krok „Cofnij”, intencja dla UI, oznaczenie niezaufanej treści);
//! - [`BrokerGate`]: protokół Brokera `decide` → `Allow(token)` | `NeedsApproval` (czekanie
//!   z limitem i anulowaniem) | `Deny` → `verify` przy użyciu → unieważnienie tokenu;
//! - [`paths`]: ścieżki od modelu bez postaci niejednoznacznych (`..`, ADS, urządzenia);
//! - [`text`]: redakcja sekretów, obcinanie, delimitacja niezaufanej treści w prompcie;
//! - [`ScriptedTool`]: deterministyczna atrapa (podstawa `tools-*-fake`).

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

mod call;
mod gate;
mod manifest;
pub mod paths;
mod scripted;
pub mod text;

#[cfg(feature = "contract-tests")]
pub mod contract_tests;

pub use call::{
    DEFAULT_APPROVAL_TIMEOUT, DenialReason, Tool, ToolCtx, ToolErrorKind, ToolImage, ToolIntent,
    ToolObserver, ToolOutcome, ToolStatus, UndoRef, UndoService, parse_args,
};
pub use gate::{Authorization, BrokerGate, DEFAULT_APPROVAL_POLL, GateError};
pub use manifest::{ManifestError, ToolManifest, schema_of};
pub use scripted::{RecordedCall, ScriptedTool};

use core_bus_contract::{Event, EventKind, Level};
use safety_broker_contract::{ActionRequest, Capability, DeclaredFacts, TaintSource};

/// Zestaw narzędzi modułu (`tools-fs`, `tools-shell`, `tools-clipboard`) do rejestru runtime.
pub trait Toolset: Send + Sync {
    /// Narzędzia zestawu (każde z własnym manifestem).
    fn tools(&self) -> Vec<std::sync::Arc<dyn Tool>>;
}

/// Rodzaj zdarzenia jako `EventKind` (konwencja `tool.<moduł>.<czynność>`).
pub fn event_kind(name: &str) -> EventKind {
    EventKind::Custom(name.to_owned())
}

/// Zdarzenie narzędzia z sesją, agentką i przebiegiem z kontekstu.
pub fn tool_event(name: &str, level: Level, payload: serde_json::Value, ctx: &ToolCtx) -> Event {
    let mut event =
        Event::new(event_kind(name), level, payload).with_session(ctx.holder.session.clone());
    if let Some(agent) = &ctx.holder.agent {
        event = event.with_agent(agent.clone());
    }
    if let Some(run) = &ctx.run {
        event = event.with_run(run.clone());
    }
    event
}

/// Fakty bazowe z manifestu i kontekstu (narzędzie uzupełnia destrukcyjność, polecenie…).
pub fn base_facts(manifest: &ToolManifest, ctx: &ToolCtx) -> DeclaredFacts {
    let mut facts = DeclaredFacts::new(&manifest.id);
    facts.reversible = manifest.reversible;
    facts.untrusted_input_in_args = ctx.untrusted_args;
    facts
}

/// Żądanie tokenu dla akcji narzędzia.
pub fn action_request(
    ctx: &ToolCtx,
    capability: Capability,
    facts: DeclaredFacts,
) -> ActionRequest {
    ActionRequest {
        holder: ctx.holder.clone(),
        capability,
        facts,
        origin: ctx.origin,
        ttl_ms: Some(TOKEN_TTL_MS),
    }
}

/// TTL tokenu narzędzia (krótki — token jest unieważniany zaraz po akcji).
pub const TOKEN_TTL_MS: u64 = 5 * 60 * 1000;

/// Zgłasza Brokerowi niezaufaną treść w sesji (taint monotoniczny); błąd nie blokuje wyniku —
/// runtime i tak traktuje treść jako niezaufaną.
pub async fn report_untrusted(gate: &BrokerGate, ctx: &ToolCtx, source: TaintSource) {
    let _ = gate
        .broker()
        .report_untrusted_input(&ctx.holder.session, source)
        .await;
}

#[cfg(test)]
mod tests {
    use super::*;
    use safety_broker_contract::Holder;

    #[test]
    fn facts_and_events_carry_context() {
        let mut ctx = ToolCtx::new(Holder::agent("s1", "delta"));
        ctx.untrusted_args = true;
        ctx.run = Some(core_bus_contract::RunId::new("r1"));
        let m = ToolManifest {
            name: "fs_write".into(),
            id: "tools-fs.write".into(),
            title: "Zapis".into(),
            description: String::new(),
            input_schema: serde_json::Value::Null,
            output_schema: serde_json::Value::Null,
            reversible: risk_classifier_contract::Reversibility::Scoped,
            capabilities: vec![],
            groups: vec![],
            mutating: true,
            untrusted_output: None,
        };
        let f = base_facts(&m, &ctx);
        assert_eq!(f.tool, "tools-fs.write");
        assert!(f.untrusted_input_in_args);
        assert_eq!(
            f.reversible,
            risk_classifier_contract::Reversibility::Scoped
        );
        let cap = Capability::FsRead(paths::exact_scope("C:\\x", &Default::default()).unwrap());
        let req = action_request(&ctx, cap, f);
        assert_eq!(req.ttl_ms, Some(TOKEN_TTL_MS));
        let e = tool_event("tool.fs.called", Level::Info, serde_json::json!({}), &ctx);
        assert_eq!(e.kind.as_str(), "tool.fs.called");
        assert_eq!(e.agent.unwrap().as_str(), "delta");
        assert_eq!(e.run.unwrap().as_str(), "r1");
    }
}
