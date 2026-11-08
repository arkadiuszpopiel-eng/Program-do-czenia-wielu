//! `tools-uia` — implementacja (docs/modules/tools-uia/SPEC.md, PLAN §7.1–7.3, §8.7).
//!
//! Każde wywołanie: okno celu (odmowa dla okien Alfy/Brokera przed Brokerem) → Broker
//! `gui.control(<aplikacja>)` → `UiaPort` na wątku blokującym (port ma własne limity czasu i
//! strażnika). Odczyty: sekrety redagowane, pola haseł bez wartości, wynik niezaufany (taint
//! `Screen`, zgłoszony Brokerowi). Akcje: stan przed i po → weryfikacja (`tool.gui.verify`).

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

use std::sync::Arc;

use async_trait::async_trait;
use core_bus_contract::EventBus;
use core_registry_contract::{ManifestError, ModuleManifest};
use platform_contract::{
    DesktopPort, ElementRef, ExpandState, GuiError, TreeOptions, UiaAction, UiaNode, UiaPort,
    WindowId,
};
use safety_broker_contract::{Broker, TaintSource};
use tools_common_contract::{
    Authorization, BrokerGate, Tool, ToolCtx, ToolErrorKind, ToolManifest, ToolOutcome, Toolset,
    parse_args, report_untrusted, text,
};
use tools_uia_contract::{
    ActArgs, ActOutput, EVENT_ACT, EVENT_READ, FindArgs, FindOutput, TextArgs, TextOutput,
    TreeArgs, TreeOutput, UiaToolsConfig, manifests, node_out, render_nodes, to_action, to_query,
};
use tools_window_contract::gui::{self, Step};

/// Treść `module.toml` tego modułu.
pub const MODULE_TOML: &str = include_str!("../module.toml");

/// Manifest modułu (rejestr).
pub fn module_manifest() -> Result<ModuleManifest, ManifestError> {
    ModuleManifest::parse_toml(MODULE_TOML)
}

/// Zależności narzędzi UIA.
#[derive(Clone)]
pub struct UiaToolsDeps {
    /// Port okien (cel i aplikacja).
    pub desktop: Arc<dyn DesktopPort>,
    /// Port UI Automation.
    pub uia: Arc<dyn UiaPort>,
    /// Broker.
    pub broker: Arc<dyn Broker>,
    /// Limity.
    pub config: UiaToolsConfig,
    /// Magistrala.
    pub bus: Option<Arc<dyn EventBus>>,
}

struct Core {
    desktop: Arc<dyn DesktopPort>,
    uia: Arc<dyn UiaPort>,
    gate: BrokerGate,
    config: UiaToolsConfig,
    bus: Option<Arc<dyn EventBus>>,
}

fn invalid(text: String) -> Box<ToolOutcome> {
    Box::new(ToolOutcome::failed(
        ToolErrorKind::InvalidArgs,
        format!("Niepoprawne argumenty: {text}."),
    ))
}

fn port<T>(r: Result<T, GuiError>, action: &str) -> Step<T> {
    r.map_err(|e| Box::new(gui::gui_outcome(&e, action)))
}

/// Czy stan po akcji potwierdza zamiar.
fn verified(action: &UiaAction, before: &UiaNode, after: &UiaNode) -> bool {
    match action {
        UiaAction::SetValue { value } => {
            after.is_password || after.value.as_deref() == Some(value.as_str())
        }
        UiaAction::Toggle => after.toggle != before.toggle,
        UiaAction::Expand => matches!(
            after.expand,
            Some(ExpandState::Expanded | ExpandState::PartiallyExpanded)
        ),
        UiaAction::Collapse => after.expand == Some(ExpandState::Collapsed),
        UiaAction::Select => after.selected == Some(true),
        UiaAction::Invoke | UiaAction::Scroll { .. } => true,
    }
}

impl Core {
    /// Okno celu + zgoda Brokera.
    async fn admit(
        &self,
        window: WindowId,
        ctx: &ToolCtx,
        m: &ToolManifest,
        private: bool,
        action: &str,
    ) -> Step<Authorization> {
        let desktop = self.desktop.clone();
        let owned = action.to_owned();
        let target = gui::blocking(move || gui::target_window(&*desktop, window, &owned)).await??;
        let cap = gui::app_capability(&target, action)?;
        let auth = gui::authorize(&self.gate, ctx, m, &cap, private, action).await?;
        if ctx.cancel.is_cancelled() {
            self.gate.release(std::slice::from_ref(&auth)).await;
            return Err(Box::new(ToolOutcome::cancelled(action)));
        }
        Ok(auth)
    }

    async fn read_done(&self, ctx: &ToolCtx, tool: &str, window: WindowId, nodes: usize) {
        report_untrusted(&self.gate, ctx, TaintSource::Screen).await;
        gui::emit(
            self.bus.as_ref(),
            EVENT_READ,
            serde_json::json!({ "tool": tool, "window": window.0, "nodes": nodes }),
            ctx,
        )
        .await;
    }

    fn finish(&self, body: String, data: serde_json::Value, auth: &Authorization) -> ToolOutcome {
        let (body, cut) = text::truncate_chars(&body, self.config.output_max_chars);
        let mut out = ToolOutcome::ok(body, data).untrusted(TaintSource::Screen);
        out.truncated = cut;
        out.approval = auth.approval;
        out
    }

    async fn tree(
        &self,
        args: serde_json::Value,
        ctx: &ToolCtx,
        m: &ToolManifest,
    ) -> Step<ToolOutcome> {
        let a: TreeArgs = parse_args(args)?;
        let window = WindowId(a.window);
        let options = TreeOptions {
            max_depth: a.max_depth.unwrap_or(12).clamp(1, 30),
            max_nodes: usize::try_from(
                a.max_nodes.unwrap_or(self.config.max_nodes).clamp(1, 1_000),
            )
            .unwrap_or(300),
            include_offscreen: a.include_offscreen.unwrap_or(false),
        };
        let action = "odczyt drzewa UI";
        let auth = self.admit(window, ctx, m, true, action).await?;
        let uia = self.uia.clone();
        let r = gui::blocking(move || uia.tree(window, &options)).await;
        self.gate.release(std::slice::from_ref(&auth)).await;
        let tree = port(r?, action)?;
        let nodes: Vec<_> = tree.nodes.iter().map(node_out).collect();
        let data = TreeOutput {
            window: a.window,
            truncated: tree.truncated,
            sparse: tree.is_sparse(),
            nodes,
        };
        self.read_done(ctx, &m.name, window, data.nodes.len()).await;
        let hint = if data.sparse {
            "\nDrzewo jest ubogie — użyj `screen_capture` (trasa wizji)."
        } else {
            ""
        };
        let body = format!(
            "Drzewo UI okna {} ({} elementów{}):\n{}{hint}",
            a.window,
            data.nodes.len(),
            if data.truncated { ", obcięte" } else { "" },
            render_nodes(&data.nodes)
        );
        Ok(self.finish(body, serde_json::to_value(&data).unwrap_or_default(), &auth))
    }

    async fn find(
        &self,
        args: serde_json::Value,
        ctx: &ToolCtx,
        m: &ToolManifest,
    ) -> Step<ToolOutcome> {
        let a: FindArgs = parse_args(args)?;
        let query = to_query(&a, 50).map_err(invalid)?;
        let window = WindowId(a.window);
        let action = "wyszukiwanie w UI";
        let auth = self.admit(window, ctx, m, true, action).await?;
        let uia = self.uia.clone();
        let r = gui::blocking(move || uia.find(window, &query)).await;
        self.gate.release(std::slice::from_ref(&auth)).await;
        let found = port(r?, action)?;
        let data = FindOutput {
            window: a.window,
            matches: found.iter().map(node_out).collect(),
        };
        self.read_done(ctx, &m.name, window, data.matches.len())
            .await;
        let body = format!(
            "Znalezione elementy ({}):\n{}",
            data.matches.len(),
            render_nodes(&data.matches)
        );
        Ok(self.finish(body, serde_json::to_value(&data).unwrap_or_default(), &auth))
    }

    async fn read_text(
        &self,
        args: serde_json::Value,
        ctx: &ToolCtx,
        m: &ToolManifest,
    ) -> Step<ToolOutcome> {
        let a: TextArgs = parse_args(args)?;
        let element = ElementRef::parse(&a.element).map_err(|e| invalid(e.to_string()))?;
        let max = usize::try_from(
            a.max_chars
                .unwrap_or(self.config.text_max_chars)
                .clamp(1, 200_000),
        )
        .unwrap_or(20_000);
        let action = "odczyt tekstu z UI";
        let auth = self.admit(element.window, ctx, m, true, action).await?;
        let (uia, el) = (self.uia.clone(), element.clone());
        let r = gui::blocking(move || uia.read_text(&el, max)).await;
        self.gate.release(std::slice::from_ref(&auth)).await;
        let t = port(r?, action)?;
        let data = TextOutput {
            element: a.element,
            text: text::redact_secrets(&t.text),
            truncated: t.truncated,
        };
        self.read_done(ctx, &m.name, element.window, 1).await;
        let body = format!("Tekst elementu {}:\n{}", data.element, data.text);
        let mut out = self.finish(body, serde_json::to_value(&data).unwrap_or_default(), &auth);
        out.truncated |= data.truncated;
        Ok(out)
    }

    async fn act(
        &self,
        args: serde_json::Value,
        ctx: &ToolCtx,
        m: &ToolManifest,
    ) -> Step<ToolOutcome> {
        let a: ActArgs = parse_args(args)?;
        let action = to_action(&a).map_err(invalid)?;
        let element = ElementRef::parse(&a.element).map_err(|e| invalid(e.to_string()))?;
        let what = format!("akcja UI „{}”", action.name());
        let auth = self.admit(element.window, ctx, m, false, &what).await?;
        let (uia, el, act) = (self.uia.clone(), element.clone(), action.clone());
        let r = gui::blocking(move || {
            let before = uia.element(&el)?;
            uia.act(&el, &act).map(|after| (before, after))
        })
        .await;
        self.gate.release(std::slice::from_ref(&auth)).await;
        let (before, after) = port(r?, &what)?;
        let ok = verified(&action, &before, &after);
        gui::emit_verify(
            self.bus.as_ref(),
            ctx,
            &m.name,
            element.window,
            ok,
            action.name(),
        )
        .await;
        gui::emit(self.bus.as_ref(), EVENT_ACT, serde_json::json!({ "window": element.window.0, "action": action.name(), "role": after.role }), ctx).await;
        report_untrusted(&self.gate, ctx, TaintSource::Screen).await;
        let data = ActOutput {
            element: a.element.clone(),
            action: a.action,
            verified: ok,
            after: node_out(&after),
        };
        let note = if ok {
            "Stan po akcji potwierdza zamiar."
        } else {
            "Stan po akcji NIE potwierdza zamiaru — sprawdź element."
        };
        let body = format!(
            "Wykonałam {what} na „{}” ({}). {note}\n{}",
            data.after.name,
            data.after.role,
            render_nodes(std::slice::from_ref(&data.after))
        );
        Ok(self.finish(body, serde_json::to_value(&data).unwrap_or_default(), &auth))
    }
}

/// Zestaw narzędzi UIA.
#[derive(Clone)]
pub struct UiaTools {
    core: Arc<Core>,
}

impl UiaTools {
    /// Zestaw nad zależnościami.
    pub fn new(deps: UiaToolsDeps) -> Self {
        Self {
            core: Arc::new(Core {
                desktop: deps.desktop,
                uia: deps.uia,
                gate: BrokerGate::new(deps.broker),
                config: deps.config,
                bus: deps.bus,
            }),
        }
    }
}

impl Toolset for UiaTools {
    fn tools(&self) -> Vec<Arc<dyn Tool>> {
        manifests()
            .into_iter()
            .map(|manifest| {
                Arc::new(UiaTool {
                    core: self.core.clone(),
                    manifest,
                }) as Arc<dyn Tool>
            })
            .collect()
    }
}

struct UiaTool {
    core: Arc<Core>,
    manifest: ToolManifest,
}

#[async_trait]
impl Tool for UiaTool {
    fn manifest(&self) -> &ToolManifest {
        &self.manifest
    }

    async fn call(&self, args: serde_json::Value, ctx: &ToolCtx) -> ToolOutcome {
        if ctx.cancel.is_cancelled() {
            return ToolOutcome::cancelled(&self.manifest.title);
        }
        let (core, m) = (&self.core, &self.manifest);
        if let Err(out) = gui::object_args(&args) {
            return *out;
        }
        let r = match m.name.as_str() {
            "uia_tree" => core.tree(args, ctx, m).await,
            "uia_find" => core.find(args, ctx, m).await,
            "uia_read_text" => core.read_text(args, ctx, m).await,
            _ => core.act(args, ctx, m).await,
        };
        r.unwrap_or_else(|out| *out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn module_manifest_parses() {
        assert_eq!(module_manifest().unwrap().id.as_str(), "tools-uia");
    }
}
