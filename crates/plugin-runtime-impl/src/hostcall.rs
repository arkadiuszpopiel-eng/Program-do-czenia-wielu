//! Funkcja hosta `alfa:plugin/host.call` — jedyne wyjście wtyczki z piaskownicy:
//! limit liczby i rozmiaru operacji → ścisłe parsowanie → zdolność ⊆ manifest wtyczki (inaczej
//! odmowa bez pytania Brokera) → `BrokerGate::authorize` dla **agentki wywołującej** (wtyczka ≤
//! jej rola i poziom autonomii; argumenty oznaczone jako niezaufane) → `verify` → port
//! wykonawczy z tokenem → unieważnienie tokenu. Odmowa wraca do wtyczki jako `err` (tekst).

use std::time::Instant;

use core_bus_contract::{Event, Level};
use plugin_runtime_contract::{
    HostError, HostOp, MAX_LOG_CHARS, MAX_LOGS, declared, event_kind, events, sanitize,
};
use risk_classifier_contract::{Destructiveness, Reversibility};
use safety_broker_contract::{ActionRequest, DeclaredFacts};
use serde_json::Value;
use tools_common_contract::text::truncate_chars;
use wasmtime::StoreContextMut;

use crate::sandbox::{HostCtx, SandboxStop, StoreData};

type HostResult = wasmtime::Result<(Result<String, String>,)>;

/// Implementacja funkcji hosta (rejestrowana w linkerze).
pub(crate) fn host_call(
    mut store: StoreContextMut<'_, StoreData>,
    (op, args): (String, String),
) -> HostResult {
    let data = store.data_mut();
    data.stats.host_calls = data.stats.host_calls.saturating_add(1);
    if data.stats.host_calls > data.limits.max_host_calls {
        data.stop = Some(SandboxStop::HostCalls);
        return Err(wasmtime::Error::new(SandboxStop::HostCalls));
    }
    if data.cancel.is_cancelled() {
        data.stop = Some(SandboxStop::Cancelled);
        return Err(wasmtime::Error::new(SandboxStop::Cancelled));
    }
    let limit = usize::try_from(data.limits.max_output_bytes).unwrap_or(usize::MAX);
    let started = Instant::now();
    let result = if op.len().saturating_add(args.len()) > limit {
        Err(HostError::TooLarge(limit))
    } else {
        execute(data, &op, &args)
    };
    data.host_time += started.elapsed();
    let result = result.and_then(|v| {
        let text = v.to_string();
        if text.len() > limit {
            Err(HostError::TooLarge(limit))
        } else {
            Ok(text)
        }
    });
    if let Err(e) = &result
        && matches!(e, HostError::NotDeclared(_) | HostError::Denied(_))
    {
        data.stats.host_denied = data.stats.host_denied.saturating_add(1);
        let event = denied_event(data, &op, e);
        data.events.push(event);
    }
    Ok((result.map_err(|e| e.to_string()),))
}

fn denied_event(data: &StoreData, op: &str, e: &HostError) -> Event {
    let mut event = Event::new(
        event_kind(events::HOST_DENIED),
        Level::Warn,
        serde_json::json!({
            "plugin": data.stats.plugin,
            "version": data.stats.version,
            "tool": data.stats.tool,
            "op": sanitize(&op.chars().take(64).collect::<String>()),
            "reason": e,
        }),
    );
    if let Some(h) = &data.host {
        event = event.with_session(h.tool_ctx.holder.session.clone());
        if let Some(agent) = &h.tool_ctx.holder.agent {
            event = event.with_agent(agent.clone());
        }
    }
    event
}

fn execute(data: &mut StoreData, op: &str, args: &str) -> Result<Value, HostError> {
    let op = HostOp::parse(op, args)?;
    if let HostOp::Log { message } = &op {
        if data.stats.logs.len() < MAX_LOGS {
            let line = truncate_chars(&sanitize(message), MAX_LOG_CHARS).0;
            data.stats.logs.push(line);
        }
        return Ok(Value::Null);
    }
    let ctx = data
        .host
        .as_ref()
        .ok_or_else(|| HostError::Denied("host niedostępny podczas walidacji".into()))?;
    let needed = op
        .capability()?
        .ok_or_else(|| HostError::BadArgs("operacja bez zdolności".into()))?;
    if !declared(&needed, &ctx.declared) {
        return Err(HostError::NotDeclared(needed.to_string()));
    }
    authorize_and_run(ctx, &op, needed)
}

fn authorize_and_run(
    ctx: &HostCtx,
    op: &HostOp,
    needed: safety_broker_contract::Capability,
) -> Result<Value, HostError> {
    let mut facts = DeclaredFacts::new(&ctx.tool_id);
    if op.mutating() {
        facts.reversible = Reversibility::No;
        facts.destructive = if matches!(op, HostOp::FsWriteText { .. }) {
            Destructiveness::Recoverable
        } else {
            Destructiveness::None
        };
    }
    facts.untrusted_input_in_args = true;
    let request = ActionRequest {
        holder: ctx.tool_ctx.holder.clone(),
        capability: needed.clone(),
        facts,
        origin: ctx.tool_ctx.origin,
        ttl_ms: Some(ctx.token_ttl_ms),
    };
    let holder = ctx.tool_ctx.holder.clone();
    ctx.handle.block_on(async {
        let auth = ctx
            .gate
            .authorize(request, &ctx.tool_ctx)
            .await
            .map_err(|e| HostError::Denied(e.to_string()))?;
        let result = match ctx.gate.verify(&auth, &needed, &holder) {
            Ok(()) => ctx.host.execute(op, Some(&auth.token), &holder).await,
            Err(e) => Err(HostError::Denied(e.to_string())),
        };
        ctx.gate.release(std::slice::from_ref(&auth)).await;
        result
    })
}
