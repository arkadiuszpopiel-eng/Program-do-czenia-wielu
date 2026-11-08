//! `tools-input` — implementacja (docs/modules/tools-input/SPEC.md, PLAN §7.2–7.4, §8.2).
//!
//! Wywołanie: argumenty → limit tempa (wywołania na minutę na sesję) → okno celu (odmowa dla
//! okien Alfy/Brokera przed Brokerem) → Broker `gui.control(<aplikacja>)` → plan wejścia
//! (punkt względem okna albo środek elementu UIA, zawsze w obrębie okna) → fokus (klawiatura) →
//! `InputPort::send` na wątku blokującym (strażnik, cel i fizyczne wejście użytkownika sprawdzane
//! przed każdą paczką; anulowanie przebiegu przerywa przed następną paczką) → weryfikacja
//! (okno nadal z fokusem / pod punktem). Wpisywany tekst nie trafia do zdarzeń.
//!
//! Tekst i skróty edytujące nigdy do pola hasła (przegląd #2, P2-03): po ustawieniu fokusu
//! element z fokusem odczytany przez UIA (`UiaPort::focused`) musi być ustalony i nie być polem
//! hasła — inaczej odmowa (fail-closed); port wejścia sprawdza to ponownie przed każdą paczką.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

mod plan;

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use async_trait::async_trait;
use core_bus_contract::EventBus;
use core_registry_contract::{ManifestError, ModuleManifest};
use platform_contract::{
    DesktopPort, FocusedField, GuiError, InputControl, InputPlan, InputPort, InputReport, UiaPort,
    WindowId,
};
use safety_broker_contract::Broker;
use tools_common_contract::{
    BrokerGate, DenialReason, Tool, ToolCtx, ToolManifest, ToolOutcome, Toolset,
};
use tools_input_contract::{EVENT_SENT, InputOutput, InputToolsConfig, manifests};
use tools_window_contract::gui::{self, Step};

use plan::{Aim, Request};

/// Treść `module.toml` tego modułu.
pub const MODULE_TOML: &str = include_str!("../module.toml");

/// Manifest modułu (rejestr).
pub fn module_manifest() -> Result<ModuleManifest, ManifestError> {
    ModuleManifest::parse_toml(MODULE_TOML)
}

/// Zależności narzędzi wejścia.
#[derive(Clone)]
pub struct InputToolsDeps {
    /// Port okien (cel, fokus, weryfikacja).
    pub desktop: Arc<dyn DesktopPort>,
    /// Port UIA (kliknięcie w element).
    pub uia: Arc<dyn UiaPort>,
    /// Port wejścia syntetycznego.
    pub input: Arc<dyn InputPort>,
    /// Broker.
    pub broker: Arc<dyn Broker>,
    /// Limity.
    pub config: InputToolsConfig,
    /// Magistrala.
    pub bus: Option<Arc<dyn EventBus>>,
}

struct Core {
    deps: InputToolsDeps,
    gate: BrokerGate,
    calls: Mutex<HashMap<String, VecDeque<Instant>>>,
}

impl Core {
    fn calls(&self) -> MutexGuard<'_, HashMap<String, VecDeque<Instant>>> {
        self.calls.lock().unwrap_or_else(|p| p.into_inner())
    }

    /// Limit tempa: najwyżej `max_calls_per_minute` wywołań na sesję w oknie 60 s.
    fn rate_check(&self, ctx: &ToolCtx, action: &str) -> Step<()> {
        let now = Instant::now();
        let mut calls = self.calls();
        let q = calls
            .entry(ctx.holder.session.as_str().to_owned())
            .or_default();
        while q
            .front()
            .is_some_and(|t| now.duration_since(*t) > Duration::from_secs(60))
        {
            q.pop_front();
        }
        if q.len() >= self.deps.config.max_calls_per_minute.max(1) as usize {
            let mut o = ToolOutcome::denied(DenialReason::Policy, action);
            o.text = format!(
                "Odmowa: {action} — limit tempa wejścia ({} wywołań na minutę). Zwolnij albo użyj UI Automation.",
                self.deps.config.max_calls_per_minute
            );
            return Err(Box::new(o));
        }
        q.push_back(now);
        Ok(())
    }

    async fn send(&self, plan: InputPlan, ctx: &ToolCtx) -> Step<Result<InputReport, GuiError>> {
        let control = InputControl::new();
        let (input, c2) = (self.deps.input.clone(), control.clone());
        let mut handle = tokio::task::spawn_blocking(move || input.send(&plan, &c2));
        let joined = tokio::select! {
            r = &mut handle => r,
            () = ctx.cancel.cancelled() => {
                control.cancel();
                handle.await
            }
        };
        joined.map_err(|e| {
            Box::new(ToolOutcome::failed(
                tools_common_contract::ToolErrorKind::Internal,
                format!("Wątek wejścia: {e}."),
            ))
        })
    }

    async fn run(
        &self,
        m: &ToolManifest,
        args: serde_json::Value,
        ctx: &ToolCtx,
    ) -> Step<ToolOutcome> {
        let action = m.title.to_lowercase();
        let request = plan::parse(&m.name, args, &self.deps.config)?;
        self.rate_check(ctx, &action)?;
        let window = request.window();
        let desktop = self.deps.desktop.clone();
        let owned = action.clone();
        let target = gui::blocking(move || gui::target_window(&*desktop, window, &owned)).await??;
        let cap = gui::app_capability(&target, &action)?;
        let auth = gui::authorize(&self.gate, ctx, m, &cap, false, &action).await?;
        let result = self.execute(request, &target, ctx, &action).await;
        self.gate.release(std::slice::from_ref(&auth)).await;
        let (report, aim) = result?;
        let ok = self.verify(window, aim).await;
        gui::emit_verify(
            self.deps.bus.as_ref(),
            ctx,
            &m.name,
            window,
            ok,
            if aim.is_some() {
                "okno pod punktem"
            } else {
                "fokus"
            },
        )
        .await;
        let app = gui::brief(&target).app;
        gui::emit(self.deps.bus.as_ref(), EVENT_SENT, serde_json::json!({ "tool": m.name, "window": window.0, "app": app, "batches": report.batches, "events": report.events }), ctx).await;
        let data = InputOutput {
            window: window.0,
            app: app.clone(),
            batches: report.batches,
            events: report.events,
            verified: ok,
        };
        let note = if ok {
            ""
        } else {
            " Uwaga: po akcji okno nie jest już celem — sprawdź stan (window_list)."
        };
        let mut out = ToolOutcome::ok(
            format!(
                "{}: wysłano {} zdarzeń do {app}.{note}",
                m.title, report.events
            ),
            serde_json::to_value(&data).unwrap_or_default(),
        );
        out.approval = auth.approval;
        Ok(out)
    }

    async fn execute(
        &self,
        request: Request,
        target: &platform_contract::DesktopWindow,
        ctx: &ToolCtx,
        action: &str,
    ) -> Step<(InputReport, Option<(i32, i32)>)> {
        let uia = self.deps.uia.clone();
        let t = target.clone();
        let writes_text = request.writes_text();
        let (plan, aim) = gui::blocking(move || plan::build(request, &t, &*uia))
            .await?
            .map_err(|e| Box::new(gui::gui_outcome(&e, action)))?;
        if matches!(aim, Aim::Focus) && !target.focused {
            let (desktop, id) = (self.deps.desktop.clone(), target.id);
            gui::blocking(move || desktop.focus(id))
                .await?
                .map_err(|e| Box::new(gui::gui_outcome(&e, action)))?;
        }
        if writes_text {
            let (uia, id) = (self.deps.uia.clone(), target.id);
            let field = gui::blocking(move || FocusedField::from_lookup(&uia.focused(id))).await?;
            field
                .check_typing()
                .map_err(|e| Box::new(gui::gui_outcome(&e, action)))?;
        }
        if ctx.cancel.is_cancelled() {
            return Err(Box::new(ToolOutcome::cancelled(action)));
        }
        let report = self
            .send(plan, ctx)
            .await?
            .map_err(|e| Box::new(gui::gui_outcome(&e, action)))?;
        Ok((report, aim.point()))
    }

    async fn verify(&self, window: WindowId, point: Option<(i32, i32)>) -> bool {
        let desktop = self.deps.desktop.clone();
        gui::blocking(move || match point {
            Some((x, y)) => desktop.window_at(x, y).ok().flatten() == Some(window),
            None => desktop
                .foreground()
                .ok()
                .flatten()
                .is_some_and(|w| w.id == window),
        })
        .await
        .unwrap_or(false)
    }
}

/// Zestaw narzędzi wejścia.
#[derive(Clone)]
pub struct InputTools {
    core: Arc<Core>,
}

impl InputTools {
    /// Zestaw nad zależnościami.
    pub fn new(deps: InputToolsDeps) -> Self {
        let gate = BrokerGate::new(deps.broker.clone());
        Self {
            core: Arc::new(Core {
                deps,
                gate,
                calls: Mutex::new(HashMap::new()),
            }),
        }
    }
}

impl Toolset for InputTools {
    fn tools(&self) -> Vec<Arc<dyn Tool>> {
        manifests()
            .into_iter()
            .map(|manifest| {
                Arc::new(InputTool {
                    core: self.core.clone(),
                    manifest,
                }) as Arc<dyn Tool>
            })
            .collect()
    }
}

struct InputTool {
    core: Arc<Core>,
    manifest: ToolManifest,
}

#[async_trait]
impl Tool for InputTool {
    fn manifest(&self) -> &ToolManifest {
        &self.manifest
    }

    async fn call(&self, args: serde_json::Value, ctx: &ToolCtx) -> ToolOutcome {
        if let Err(out) = gui::object_args(&args) {
            return *out;
        }
        if ctx.cancel.is_cancelled() {
            return ToolOutcome::cancelled(&self.manifest.title);
        }
        self.core
            .run(&self.manifest, args, ctx)
            .await
            .unwrap_or_else(|out| *out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn module_manifest_parses() {
        assert_eq!(module_manifest().unwrap().id.as_str(), "tools-input");
    }
}
