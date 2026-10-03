//! Narzędzie wtyczki w rejestrze agentek (`tools-common::Tool`): kontrola wejścia, ładowanie
//! modułu z weryfikacją hasha, wywołanie w piaskownicy na wątku blokującym, deserializacja
//! wyniku (treść złośliwa = błąd), wynik niezaufany + taint sesji, zdarzenia `plugin.*`.

use std::sync::Arc;

use async_trait::async_trait;
use core_bus_contract::Event;
use plugin_runtime_contract::{
    ExecError, InvocationStats, PluginId, PluginToolDecl, UNTRUSTED_SOURCE, check_approved,
    check_input, ok_outcome, parse_output, sanitize,
};
use tools_common_contract::{Tool, ToolCtx, ToolManifest, ToolOutcome, report_untrusted};

use crate::Inner;
use crate::sandbox::{self, HostCtx, StoreData};

/// Narzędzie dostarczone przez wtyczkę.
pub struct PluginTool {
    inner: Arc<Inner>,
    plugin: PluginId,
    decl: PluginToolDecl,
    manifest: ToolManifest,
}

impl PluginTool {
    pub(crate) fn new(
        inner: Arc<Inner>,
        plugin: PluginId,
        decl: PluginToolDecl,
        manifest: ToolManifest,
    ) -> Self {
        Self {
            inner,
            plugin,
            decl,
            manifest,
        }
    }
}

fn with_ctx(mut event: Event, ctx: &ToolCtx) -> Event {
    event = event.with_session(ctx.holder.session.clone());
    if let Some(agent) = &ctx.holder.agent {
        event = event.with_agent(agent.clone());
    }
    if let Some(run) = &ctx.run {
        event = event.with_run(run.clone());
    }
    event
}

#[async_trait]
impl Tool for PluginTool {
    fn manifest(&self) -> &ToolManifest {
        &self.manifest
    }

    async fn call(&self, args: serde_json::Value, ctx: &ToolCtx) -> ToolOutcome {
        let action = format!("{} (wtyczka {})", self.manifest.title, self.plugin);
        let (produced, events) = invoke(&self.inner, &self.plugin, &self.decl, args, ctx).await;
        if let Some(source) = produced.untrusted.clone() {
            report_untrusted(&self.inner.gate, ctx, source).await;
        }
        self.inner
            .publish(events.into_iter().map(|e| with_ctx(e, ctx)).collect())
            .await;
        produced.finish(&action)
    }
}

/// Wynik z oznaczeniem, czy wtyczka wyprodukowała treść (taint).
struct Produced {
    result: Result<ToolOutcome, ExecError>,
    untrusted: Option<safety_broker_contract::TaintSource>,
}

impl Produced {
    fn err(e: ExecError) -> Self {
        Self {
            result: Err(e),
            untrusted: None,
        }
    }

    /// Wynik dla modelu: błąd wtyczki (jej tekst) też jest treścią niezaufaną.
    fn finish(self, action: &str) -> ToolOutcome {
        let mut out = self.result.unwrap_or_else(|e| e.to_outcome(action));
        if out.untrusted.is_none() {
            out.untrusted = self.untrusted;
        }
        out
    }
}

async fn invoke(
    inner: &Arc<Inner>,
    id: &PluginId,
    decl: &PluginToolDecl,
    args: serde_json::Value,
    ctx: &ToolCtx,
) -> (Produced, Vec<Event>) {
    let Some(record) = inner.lib().installed(id).cloned() else {
        return (
            Produced::err(ExecError::Unavailable(id.to_string())),
            Vec::new(),
        );
    };
    // Integralność zatwierdzenia przy każdym wywołaniu (nie tylko przy ładowaniu modułu):
    // pamięć podręczna jest kluczowana hashem bajtów, a deklaracje zdolności są w rekordzie.
    if let Err(e) = check_approved(&record) {
        return (Produced::err(ExecError::Load(e)), Vec::new());
    }
    if record.manifest.tool(&decl.name).is_none() {
        return (
            Produced::err(ExecError::Unavailable(decl.name.clone())),
            Vec::new(),
        );
    }
    let limits = record.manifest.limits;
    let input = args.to_string();
    if let Err(e) = check_input(&input, &limits) {
        return (Produced::err(e), Vec::new());
    }
    let pre = match inner.prepared(&record).await {
        Ok(p) => p,
        Err(e) => return (Produced::err(ExecError::Load(e)), Vec::new()),
    };
    let Ok(_permit) = inner.permits.acquire().await else {
        return (
            Produced::err(ExecError::Internal("piaskownica zamknięta".into())),
            Vec::new(),
        );
    };
    if ctx.cancel.is_cancelled() {
        return (Produced::err(ExecError::Cancelled), Vec::new());
    }
    let host = HostCtx {
        gate: inner.gate.clone(),
        host: inner.host.clone(),
        tool_ctx: ctx.clone(),
        declared: record.manifest.capabilities.clone(),
        tool_id: format!("plugin.{id}.{}", decl.name),
        handle: tokio::runtime::Handle::current(),
        token_ttl_ms: inner.config.token_ttl_ms,
    };
    let stats = InvocationStats::new(&record.manifest, &decl.name);
    let data = StoreData::new(limits, Some(host), stats);
    let tool = decl.name.clone();
    let joined = tokio::task::spawn_blocking(move || sandbox::run(&pre, data, &tool, &input)).await;
    let run = match joined {
        Ok(r) => r,
        Err(e) => {
            return (
                Produced::err(ExecError::Internal(e.to_string())),
                Vec::new(),
            );
        }
    };
    let mut stats = run.stats;
    let produced = match run.output {
        Ok(Ok(out)) => match parse_output(&out, decl, &limits) {
            Ok(value) => Produced {
                result: Ok(ok_outcome(value)),
                untrusted: Some(UNTRUSTED_SOURCE),
            },
            Err(e) => Produced::err(e),
        },
        Ok(Err(msg)) => Produced {
            result: Err(ExecError::PluginFailed(sanitize(&msg))),
            untrusted: Some(UNTRUSTED_SOURCE),
        },
        Err(e) => Produced::err(e),
    };
    let error = produced.result.as_ref().err();
    if let Some(e) = error {
        stats.result = e.kind().into();
    }
    let mut events = run.events;
    events.push(stats.event(error));
    (produced, events)
}
