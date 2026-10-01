//! `tools-window` — implementacja (docs/modules/tools-window/SPEC.md, PLAN §7.2, §8.2).
//!
//! Lista okien: Broker (`gui.control(desktop.exe)`) → `DesktopPort` → okna chronione pominięte,
//! tytuły zredagowane i oznaczone jako niezaufane (taint). Zmiany (fokus, położenie, stan):
//! okno celu (odmowa dla chronionego przed Brokerem) → `gui.control(<aplikacja>)` → port (strażnik
//! jeszcze raz tuż przed wywołaniem) → weryfikacja stanu po zmianie (`tool.gui.verify`).

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use core_bus_contract::EventBus;
use core_registry_contract::{ManifestError, ModuleManifest};
use platform_contract::{DesktopPort, DesktopWindow, ScreenRect, WindowId, WindowState};
use safety_broker_contract::{Broker, TaintSource};
use tools_common_contract::{
    BrokerGate, Tool, ToolCtx, ToolManifest, ToolOutcome, Toolset, parse_args, report_untrusted,
    text,
};
use tools_window_contract::gui::{self, Step};
use tools_window_contract::{
    ChangeOutput, EVENT_CHANGE, EVENT_LIST, FocusArgs, ListArgs, ListOutput, MonitorBrief,
    MoveArgs, StateArg, StateArgs, manifests,
};

/// Treść `module.toml` tego modułu.
pub const MODULE_TOML: &str = include_str!("../module.toml");
/// Jak długo czekać na potwierdzenie zmiany okna (zmiany są asynchroniczne).
const VERIFY_WAIT: Duration = Duration::from_millis(1_000);
/// Tolerancja położenia przy weryfikacji (px; ramki DWM).
const VERIFY_TOLERANCE: i32 = 8;

/// Manifest modułu (rejestr).
pub fn module_manifest() -> Result<ModuleManifest, ManifestError> {
    ModuleManifest::parse_toml(MODULE_TOML)
}

/// Zależności narzędzi okien.
#[derive(Clone)]
pub struct WindowToolsDeps {
    /// Port okien v2.
    pub desktop: Arc<dyn DesktopPort>,
    /// Broker.
    pub broker: Arc<dyn Broker>,
    /// Magistrala.
    pub bus: Option<Arc<dyn EventBus>>,
}

struct Core {
    desktop: Arc<dyn DesktopPort>,
    gate: BrokerGate,
    bus: Option<Arc<dyn EventBus>>,
}

#[derive(Debug, Clone, Copy)]
enum Change {
    Focus,
    Bounds(ScreenRect),
    State(WindowState),
}

impl Change {
    fn name(self) -> &'static str {
        match self {
            Self::Focus => "focus",
            Self::Bounds(_) => "bounds",
            Self::State(_) => "state",
        }
    }

    fn done(self, w: &DesktopWindow) -> bool {
        let near = |a: i32, b: i32| (a - b).abs() <= VERIFY_TOLERANCE;
        match self {
            Self::Focus => w.focused,
            Self::Bounds(r) => {
                near(w.rect.left, r.left)
                    && near(w.rect.top, r.top)
                    && near(w.rect.right, r.right)
                    && near(w.rect.bottom, r.bottom)
            }
            Self::State(s) => w.state == s,
        }
    }
}

fn platform_failure(e: &platform_contract::GuiError, action: &str) -> Box<ToolOutcome> {
    Box::new(gui::gui_outcome(e, action))
}

impl Core {
    async fn list(
        &self,
        args: serde_json::Value,
        ctx: &ToolCtx,
        m: &ToolManifest,
    ) -> Step<ToolOutcome> {
        let a: ListArgs = parse_args(args)?;
        let action = "lista okien";
        let cap = gui::desktop_capability()?;
        let auth = gui::authorize(&self.gate, ctx, m, &cap, true, action).await?;
        let desktop = self.desktop.clone();
        let read = gui::blocking(move || {
            Ok::<_, platform_contract::GuiError>((desktop.windows()?, desktop.monitors()?))
        })
        .await;
        self.gate.release(std::slice::from_ref(&auth)).await;
        let (windows, monitors) = read?.map_err(|e| platform_failure(&e, action))?;
        let include_min = a.include_minimized.unwrap_or(true);
        let hidden = windows.iter().filter(|w| w.protected).count();
        let visible: Vec<&DesktopWindow> = windows
            .iter()
            .filter(|w| !w.protected && !w.title.trim().is_empty())
            .filter(|w| include_min || w.state != WindowState::Minimized)
            .collect();
        let briefs: Vec<_> = visible
            .iter()
            .map(|w| {
                let mut b = gui::brief(w);
                b.title = text::redact_secrets(&b.title);
                b
            })
            .collect();
        let data = ListOutput {
            foreground: briefs.iter().find(|b| b.focused).map(|b| b.window),
            windows: briefs,
            monitors: monitors
                .iter()
                .map(|m| MonitorBrief {
                    index: m.index,
                    x: m.rect.left,
                    y: m.rect.top,
                    width: m.rect.width(),
                    height: m.rect.height(),
                    dpi: m.dpi,
                    primary: m.primary,
                })
                .collect(),
            hidden_protected: u32::try_from(hidden).unwrap_or(u32::MAX),
        };
        report_untrusted(&self.gate, ctx, TaintSource::Screen).await;
        gui::emit(
            self.bus.as_ref(),
            EVENT_LIST,
            serde_json::json!({ "count": data.windows.len(), "hidden_protected": hidden }),
            ctx,
        )
        .await;
        let lines: Vec<String> = data
            .windows
            .iter()
            .map(|b| {
                format!(
                    "- [{}] {} „{}” ({},{} {}×{}) {}{}",
                    b.window,
                    b.app,
                    b.title,
                    b.x,
                    b.y,
                    b.width,
                    b.height,
                    b.state,
                    if b.focused { ", fokus" } else { "" }
                )
            })
            .collect();
        let (body, cut) = text::truncate_chars(&lines.join("\n"), 20_000);
        let mut out = ToolOutcome::ok(
            format!("Okna ({}):\n{body}", data.windows.len()),
            serde_json::to_value(&data).unwrap_or_default(),
        )
        .untrusted(TaintSource::Screen);
        out.truncated = cut;
        out.approval = auth.approval;
        Ok(out)
    }

    async fn change(
        &self,
        id: WindowId,
        change: Change,
        ctx: &ToolCtx,
        m: &ToolManifest,
    ) -> Step<ToolOutcome> {
        let action = m.title.to_lowercase();
        let desktop = self.desktop.clone();
        let before =
            gui::blocking(move || gui::target_window(&*desktop, id, "zmiana okna")).await??;
        let cap = gui::app_capability(&before, &action)?;
        let auth = gui::authorize(&self.gate, ctx, m, &cap, false, &action).await?;
        if ctx.cancel.is_cancelled() {
            self.gate.release(std::slice::from_ref(&auth)).await;
            return Ok(ToolOutcome::cancelled(&action));
        }
        let desktop = self.desktop.clone();
        let done = gui::blocking(move || match change {
            Change::Focus => desktop.focus(id),
            Change::Bounds(r) => desktop.set_bounds(id, r),
            Change::State(s) => desktop.set_state(id, s),
        })
        .await;
        self.gate.release(std::slice::from_ref(&auth)).await;
        done?.map_err(|e| platform_failure(&e, &action))?;
        let after = self.wait_for(id, change).await;
        let verified = after.as_ref().is_some_and(|w| change.done(w));
        gui::emit_verify(self.bus.as_ref(), ctx, &m.name, id, verified, change.name()).await;
        gui::emit(self.bus.as_ref(), EVENT_CHANGE, serde_json::json!({ "window": id.0, "app": gui::brief(&before).app, "change": change.name() }), ctx).await;
        let redacted = |w: &DesktopWindow| {
            let mut b = gui::brief(w);
            b.title = text::redact_secrets(&b.title);
            b
        };
        let data = ChangeOutput {
            before: redacted(&before),
            after: after.as_ref().map_or_else(|| redacted(&before), redacted),
            verified,
        };
        let note = if verified {
            "Zmiana potwierdzona."
        } else {
            "Nie potwierdzono zmiany — sprawdź okno (window_list)."
        };
        let mut out = ToolOutcome::ok(
            format!("{}: okno {} ({}). {note}", m.title, id.0, data.before.app),
            serde_json::to_value(&data).unwrap_or_default(),
        )
        .untrusted(TaintSource::Screen);
        out.approval = auth.approval;
        Ok(out)
    }

    /// Czeka (do [`VERIFY_WAIT`]) aż okno osiągnie zamierzony stan; zwraca ostatni odczyt.
    async fn wait_for(&self, id: WindowId, change: Change) -> Option<DesktopWindow> {
        let deadline = tokio::time::Instant::now() + VERIFY_WAIT;
        loop {
            let desktop = self.desktop.clone();
            let now = gui::blocking(move || desktop.window(id).ok())
                .await
                .ok()
                .flatten();
            if now.as_ref().is_none_or(|w| change.done(w))
                || tokio::time::Instant::now() >= deadline
            {
                return now;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    }
}

/// Zestaw narzędzi okien.
#[derive(Clone)]
pub struct WindowTools {
    core: Arc<Core>,
}

impl WindowTools {
    /// Zestaw nad zależnościami.
    pub fn new(deps: WindowToolsDeps) -> Self {
        Self {
            core: Arc::new(Core {
                desktop: deps.desktop,
                gate: BrokerGate::new(deps.broker),
                bus: deps.bus,
            }),
        }
    }
}

impl Toolset for WindowTools {
    fn tools(&self) -> Vec<Arc<dyn Tool>> {
        manifests()
            .into_iter()
            .map(|manifest| {
                Arc::new(WindowTool {
                    core: self.core.clone(),
                    manifest,
                }) as Arc<dyn Tool>
            })
            .collect()
    }
}

struct WindowTool {
    core: Arc<Core>,
    manifest: ToolManifest,
}

impl WindowTool {
    async fn run(&self, args: serde_json::Value, ctx: &ToolCtx) -> Step<ToolOutcome> {
        let m = &self.manifest;
        match m.name.as_str() {
            "window_list" => self.core.list(args, ctx, m).await,
            "window_focus" => {
                let a: FocusArgs = parse_args(args)?;
                self.core
                    .change(WindowId(a.window), Change::Focus, ctx, m)
                    .await
            }
            "window_move" => {
                let a: MoveArgs = parse_args(args)?;
                let rect = ScreenRect::from_xywh(a.x, a.y, a.width, a.height);
                self.core
                    .change(WindowId(a.window), Change::Bounds(rect), ctx, m)
                    .await
            }
            _ => {
                let a: StateArgs = parse_args(args)?;
                let s = match a.state {
                    StateArg::Minimized => WindowState::Minimized,
                    StateArg::Maximized => WindowState::Maximized,
                    StateArg::Normal => WindowState::Normal,
                };
                self.core
                    .change(WindowId(a.window), Change::State(s), ctx, m)
                    .await
            }
        }
    }
}

#[async_trait]
impl Tool for WindowTool {
    fn manifest(&self) -> &ToolManifest {
        &self.manifest
    }

    async fn call(&self, args: serde_json::Value, ctx: &ToolCtx) -> ToolOutcome {
        if ctx.cancel.is_cancelled() {
            return ToolOutcome::cancelled(&self.manifest.title);
        }
        if let Err(out) = gui::object_args(&args) {
            return *out;
        }
        self.run(args, ctx).await.unwrap_or_else(|out| *out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn module_manifest_parses() {
        assert_eq!(module_manifest().unwrap().id.as_str(), "tools-window");
    }
}
