//! Narzędzia serwera MCP Alfy v1 (F6, PLAN §8.5, §9.7): UI Automation i zrzuty przez **te same
//! narzędzia agentek** (`tools-uia`, `tools-screen` — Broker `gui.control`, strażnik celów,
//! maskowanie, taint), wstrzyknięte przez kompozycję aplikacji, oraz rejestr tylko do odczytu
//! (`RegistryPort` + Broker). Podmiot Brokera to „most CLI” (opaque worker): agentka `most-cli`,
//! źródło `Agent`, argumenty traktowane jak pochodzące z niezaufanej treści. Każdy wynik niesie
//! `unverified_by_alfa = true`.

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;

use core_bus_contract::{AgentId, EventBus, EventKind, Level, SessionId};
use mcp_contract::{AlfaTool, CallToolResult, Content, ToolCallError, UNVERIFIED_FIELD};
use platform_apps_contract::{RegKey, RegistryError, RegistryPort, check_key};
use risk_classifier_contract::CommandOrigin;
use safety_broker_contract::{AdminOp, Broker, Capability, DeclaredFacts, Holder, TaintSource};
use serde::Deserialize;
use serde_json::{Value, json};
use tokio_util::sync::CancellationToken;
use tools_common_contract::{
    BrokerGate, Tool, ToolCtx, ToolErrorKind, ToolOutcome, ToolStatus, action_request,
    report_untrusted, text,
};

/// Agentka-podmiot wywołań mostu w Brokerze.
pub const BRIDGE_AGENT: &str = "most-cli";
/// Identyfikator narzędzia rejestru w faktach Brokera.
pub const REGISTRY_TOOL_ID: &str = "mcp.registry_read";
/// Zdarzenie: wywołanie narzędzia v1 przez most (bez treści wyniku).
pub const EVENT_CALL: &str = "mcp.alfa.call";

/// Zależności narzędzi v1 (kompozycja aplikacji).
#[derive(Clone)]
pub struct McpV1 {
    /// Narzędzia agentek: `uia_tree`, `uia_find`, `uia_read_text`, `uia_act`, `screen_capture`
    /// (inne nazwy i narzędzia bez `gui.control` są pomijane).
    pub gui_tools: Vec<Arc<dyn Tool>>,
    /// Rejestr tylko do odczytu.
    pub registry: Arc<dyn RegistryPort>,
    /// Broker.
    pub broker: Arc<dyn Broker>,
    /// Magistrala (`mcp.alfa.call`).
    pub bus: Option<Arc<dyn EventBus>>,
    /// Limit czekania na zgodę właściciela w Broker-UI.
    pub approval_timeout: Duration,
}

/// Kto woła (rejestracja mostu).
#[derive(Debug, Clone)]
pub(crate) struct Bridge {
    pub(crate) label: String,
    pub(crate) session: Option<String>,
    pub(crate) cancel: CancellationToken,
}

/// Narzędzia v1 skompilowane z zależności (współdzielone przez połączenia hosta).
pub struct V1Tools {
    gui: BTreeMap<AlfaTool, Arc<dyn Tool>>,
    registry: Arc<dyn RegistryPort>,
    gate: BrokerGate,
    bus: Option<Arc<dyn EventBus>>,
    approval_timeout: Duration,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RegArgs {
    key: String,
    #[serde(default)]
    value: Option<String>,
    #[serde(default)]
    max_entries: Option<u32>,
}

/// Polecenie `reg query` równoważne odczytowi (opis zdolności dla Brokera i karty zatwierdzenia).
pub fn reg_query_command(key: &RegKey, value: Option<&str>) -> String {
    match value {
        None => format!("reg query \"{key}\""),
        Some("") => format!("reg query \"{key}\" /ve"),
        Some(v) => format!("reg query \"{key}\" /v \"{v}\""),
    }
}

fn redact(v: &mut Value) {
    match v {
        Value::String(s) => *s = text::redact_secrets(s),
        Value::Array(items) => items.iter_mut().for_each(redact),
        Value::Object(map) => map.values_mut().for_each(redact),
        _ => {}
    }
}

fn mark(mut data: Value) -> Value {
    if !data.is_object() {
        data = json!({ "data": data });
    }
    if let Some(obj) = data.as_object_mut() {
        obj.insert(UNVERIFIED_FIELD.into(), Value::Bool(true));
    }
    data
}

/// Wynik narzędzia agentki → wynik MCP (odmowy polityki → -32001, złe argumenty → -32602).
pub(crate) fn to_result(out: ToolOutcome) -> Result<CallToolResult, ToolCallError> {
    match &out.status {
        ToolStatus::Ok => {}
        ToolStatus::Failed {
            error: ToolErrorKind::InvalidArgs,
        } => return Err(ToolCallError::InvalidParams(out.text)),
        ToolStatus::Denied { .. } => return Err(ToolCallError::Unauthorized(out.text)),
        _ => {
            return Ok(CallToolResult {
                content: vec![Content::text(out.text)],
                is_error: true,
                structured_content: Some(mark(json!({ "status": out.status }))),
            });
        }
    }
    let mut content = vec![Content::text(out.text)];
    content.extend(out.images.iter().map(|i| Content::Image {
        data: i.data_base64.clone(),
        mime_type: i.media_type.clone(),
    }));
    Ok(CallToolResult {
        content,
        is_error: false,
        structured_content: Some(mark(out.data)),
    })
}

impl V1Tools {
    /// Narzędzia z zależności (pomija narzędzia o nazwach spoza v1 i bez `gui.control`).
    pub fn new(v1: McpV1) -> Self {
        let gui = v1
            .gui_tools
            .into_iter()
            .filter(|t| t.manifest().capabilities.iter().any(|c| c == "gui.control"))
            .filter_map(|t| {
                AlfaTool::from_name(&t.manifest().name)
                    .filter(|a| a.is_v1() && *a != AlfaTool::RegistryRead)
                    .map(|a| (a, t))
            })
            .collect();
        Self {
            gui,
            registry: v1.registry,
            gate: BrokerGate::new(v1.broker),
            bus: v1.bus,
            approval_timeout: v1.approval_timeout,
        }
    }

    /// Czy narzędzie v1 jest dostępne (rejestr zawsze; UIA/zrzut — gdy wstrzyknięto narzędzie).
    pub fn available(&self, t: AlfaTool) -> bool {
        t == AlfaTool::RegistryRead || self.gui.contains_key(&t)
    }

    fn ctx(&self, who: &Bridge) -> ToolCtx {
        let session = who
            .session
            .clone()
            .unwrap_or_else(|| format!("most:{}", who.label));
        let mut ctx = ToolCtx::new(Holder {
            session: SessionId::new(&session),
            agent: Some(AgentId::new(BRIDGE_AGENT)),
            role: Some(BRIDGE_AGENT.into()),
        });
        ctx.origin = CommandOrigin::Agent;
        ctx.untrusted_args = true;
        ctx.label = format!("Most CLI: {}", who.label);
        ctx.approval_timeout = self.approval_timeout;
        ctx.cancel = who.cancel.clone();
        ctx
    }

    async fn emit(&self, ctx: &ToolCtx, tool: AlfaTool, ok: bool, who: &Bridge) {
        if let Some(bus) = &self.bus {
            let payload = json!({
                "tool": tool.name(), "ok": ok, "origin": BRIDGE_AGENT, "label": who.label,
                UNVERIFIED_FIELD: true,
            });
            let mut e = core_bus_contract::Event::new(
                EventKind::Custom(EVENT_CALL.into()),
                Level::Info,
                payload,
            )
            .with_session(ctx.holder.session.clone());
            if let Some(a) = &ctx.holder.agent {
                e = e.with_agent(a.clone());
            }
            let _ = bus.publish(e).await;
        }
    }

    /// Wywołanie narzędzia v1 w imieniu mostu.
    pub(crate) async fn call(
        &self,
        tool: AlfaTool,
        args: Value,
        who: &Bridge,
    ) -> Result<CallToolResult, ToolCallError> {
        let ctx = self.ctx(who);
        let result = if tool == AlfaTool::RegistryRead {
            self.registry_read(args, &ctx).await
        } else {
            let t = self
                .gui
                .get(&tool)
                .ok_or_else(|| ToolCallError::Unknown(tool.name().into()))?;
            to_result(t.call(args, &ctx).await)
        };
        self.emit(&ctx, tool, result.as_ref().is_ok_and(|r| !r.is_error), who)
            .await;
        result
    }

    async fn registry_read(
        &self,
        args: Value,
        ctx: &ToolCtx,
    ) -> Result<CallToolResult, ToolCallError> {
        let a: RegArgs = serde_json::from_value(args)
            .map_err(|e| ToolCallError::InvalidParams(format!("niepoprawne argumenty: {e}")))?;
        let key = RegKey::parse(&a.key).map_err(|e| ToolCallError::InvalidParams(e.to_string()))?;
        check_key(&key).map_err(|e| ToolCallError::Unauthorized(e.to_string()))?;
        if a.key.contains('"') || a.value.as_deref().is_some_and(|v| v.contains('"')) {
            return Err(ToolCallError::InvalidParams(
                "cudzysłów w nazwie klucza lub wartości".into(),
            ));
        }
        let action = format!("odczyt rejestru {key}");
        let cap = Capability::SystemAdmin(AdminOp::Other {
            command: reg_query_command(&key, a.value.as_deref()),
        });
        let mut facts = DeclaredFacts::new(REGISTRY_TOOL_ID);
        facts.touches_private_data = true;
        facts.untrusted_input_in_args = ctx.untrusted_args;
        let auth = match self
            .gate
            .authorize(action_request(ctx, cap.clone(), facts), ctx)
            .await
        {
            Ok(a) => a,
            Err(e) => return to_result(e.into_outcome(&action)),
        };
        if let Err(e) = self.gate.verify(&auth, &cap, &ctx.holder) {
            self.gate.release(std::slice::from_ref(&auth)).await;
            return to_result(e.into_outcome(&action));
        }
        let (registry, k, value) = (self.registry.clone(), key.clone(), a.value.clone());
        let max = a.max_entries.unwrap_or(200).clamp(1, 500) as usize;
        let read = tokio::task::spawn_blocking(move || match value {
            Some(v) => registry
                .read_value(&k, &v)
                .map(|v| serde_json::to_value(v).unwrap_or_default()),
            None => registry
                .list(&k, max)
                .map(|l| serde_json::to_value(l).unwrap_or_default()),
        })
        .await;
        self.gate.release(std::slice::from_ref(&auth)).await;
        let mut data = match read {
            Ok(Ok(v)) => v,
            Ok(Err(RegistryError::Denied(m))) => return Err(ToolCallError::Unauthorized(m)),
            Ok(Err(e)) => return Ok(CallToolResult::failure(format!("{action}: {e}"))),
            Err(e) => return Ok(CallToolResult::failure(format!("{action}: {e}"))),
        };
        redact(&mut data);
        report_untrusted(&self.gate, ctx, TaintSource::File).await;
        let mut out = ToolOutcome::ok(format!("Rejestr {key}: {data}"), data);
        out.approval = auth.approval;
        to_result(out)
    }
}
