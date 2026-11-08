//! Backend Windows: łączy moduły FFI (okna, UIA na wątku MTA, `SendInput` + hook, zrzuty GDI)
//! w metody wołane przez porty `WinGui`.

use std::time::{Duration, Instant};

use platform_contract::{
    CAPTURE_ATTEMPTS, CaptureRequest, DesktopWindow, ElementRef, GuiError, InputControl, InputPlan,
    InputReport, MonitorInfo, ScreenRect, Screenshot, TreeOptions, UiaAction, UiaNode, UiaQuery,
    UiaText, UiaTree, WindowId, WindowState, capture_set_stable, execute_input, union_for_mask,
    unstable_masks,
};

use crate::GuiConfig;
use crate::hook::ActivityMonitor;
use crate::input::WinInputBackend;
use crate::uia::UiaHost;
use crate::{capture, desktop, uia};

/// Wewnętrzny termin zadania UIA: 80% limitu (przejście kończy się samo, zanim host porzuci wątek).
fn deadline(timeout_ms: u64) -> Instant {
    Instant::now() + Duration::from_millis(timeout_ms.saturating_mul(4) / 5)
}

/// Backend Windows.
#[derive(Debug)]
pub(crate) struct Backend {
    uia: UiaHost,
    activity: ActivityMonitor,
}

impl Backend {
    pub(crate) fn new(config: &GuiConfig) -> Self {
        Self {
            uia: UiaHost::new(config.uia_call_timeout_ms, config.max_hung_uia_threads),
            activity: ActivityMonitor::default(),
        }
    }

    pub(crate) fn windows(&self, c: &GuiConfig) -> Result<Vec<DesktopWindow>, GuiError> {
        Ok(desktop::windows(&c.guard))
    }

    pub(crate) fn foreground(&self, c: &GuiConfig) -> Result<Option<DesktopWindow>, GuiError> {
        Ok(desktop::foreground(&c.guard))
    }

    pub(crate) fn window_at(&self, x: i32, y: i32) -> Result<Option<WindowId>, GuiError> {
        Ok(desktop::window_at(x, y))
    }

    pub(crate) fn monitors(&self) -> Result<Vec<MonitorInfo>, GuiError> {
        Ok(desktop::monitors())
    }

    pub(crate) fn focus(&self, c: &GuiConfig, id: WindowId) -> Result<(), GuiError> {
        desktop::focus(&c.guard, id)
    }

    pub(crate) fn set_bounds(
        &self,
        c: &GuiConfig,
        id: WindowId,
        rect: ScreenRect,
    ) -> Result<(), GuiError> {
        desktop::set_bounds(&c.guard, id, rect)
    }

    pub(crate) fn set_state(
        &self,
        c: &GuiConfig,
        id: WindowId,
        state: WindowState,
    ) -> Result<(), GuiError> {
        desktop::set_state(&c.guard, id, state)
    }

    pub(crate) fn uia_tree(
        &self,
        c: &GuiConfig,
        window: WindowId,
        options: &TreeOptions,
    ) -> Result<UiaTree, GuiError> {
        let (guard, options, ms) = (c.guard.clone(), *options, c.uia_tree_timeout_ms);
        self.uia.call("drzewo", ms, move |ctx| {
            uia::tree(ctx, &guard, window, &options, deadline(ms))
        })
    }

    pub(crate) fn uia_find(
        &self,
        c: &GuiConfig,
        window: WindowId,
        query: &UiaQuery,
    ) -> Result<Vec<UiaNode>, GuiError> {
        let (guard, query, ms) = (c.guard.clone(), query.clone(), c.uia_tree_timeout_ms);
        self.uia.call("wyszukiwanie", ms, move |ctx| {
            uia::find(ctx, &guard, window, &query, deadline(ms))
        })
    }

    pub(crate) fn uia_element(
        &self,
        c: &GuiConfig,
        element: &ElementRef,
    ) -> Result<UiaNode, GuiError> {
        let (guard, element, ms) = (c.guard.clone(), element.clone(), c.uia_tree_timeout_ms);
        self.uia.call("element", ms, move |ctx| {
            uia::node_of_element(ctx, &guard, &element, deadline(ms))
        })
    }

    pub(crate) fn uia_read_text(
        &self,
        c: &GuiConfig,
        element: &ElementRef,
        max: usize,
    ) -> Result<UiaText, GuiError> {
        let (guard, element, ms) = (c.guard.clone(), element.clone(), c.uia_tree_timeout_ms);
        self.uia.call("tekst", ms, move |ctx| {
            uia::read_text(ctx, &guard, &element, max, deadline(ms))
        })
    }

    pub(crate) fn uia_act(
        &self,
        c: &GuiConfig,
        element: &ElementRef,
        action: &UiaAction,
    ) -> Result<UiaNode, GuiError> {
        let (guard, element, action, ms) = (
            c.guard.clone(),
            element.clone(),
            action.clone(),
            c.uia_tree_timeout_ms,
        );
        self.uia.call(action.name(), ms, move |ctx| {
            uia::act(ctx, &guard, &element, &action, deadline(ms))
        })
    }

    pub(crate) fn uia_focused(
        &self,
        c: &GuiConfig,
        window: WindowId,
    ) -> Result<Option<UiaNode>, GuiError> {
        let guard = c.guard.clone();
        self.uia
            .call("element z fokusem", c.uia_call_timeout_ms, move |ctx| {
                uia::focused(ctx, &guard, window)
            })
    }

    pub(crate) fn uia_password_rects(
        &self,
        c: &GuiConfig,
        window: WindowId,
        ms: u64,
    ) -> Result<Vec<ScreenRect>, GuiError> {
        let guard = c.guard.clone();
        self.uia.call("pola haseł", ms, move |ctx| {
            uia::password_rects(ctx, &guard, window)
        })
    }

    pub(crate) fn send(
        &self,
        c: &GuiConfig,
        plan: &InputPlan,
        control: &InputControl,
    ) -> Result<InputReport, GuiError> {
        self.activity.ensure()?;
        let backend = WinInputBackend {
            activity: &self.activity,
            uia: &self.uia,
            uia_ms: c.uia_call_timeout_ms,
        };
        execute_input(plan, &backend, &c.guard, &c.pacing, control)
    }

    /// Zrzut: okna → klatka → okna ponownie; zbiór okien istotny dla maskowania zmienił się
    /// (np. Broker-UI wyskoczył w trakcie) → klatka ponowiona, po [`CAPTURE_ATTEMPTS`] próbach
    /// maska z sumy wyliczeń i okien zmienionych (przegląd #2, P2-02).
    pub(crate) fn capture(
        &self,
        c: &GuiConfig,
        request: &CaptureRequest,
    ) -> Result<Screenshot, GuiError> {
        let monitors = desktop::monitors();
        let per_window = c.uia_call_timeout_ms;
        let mut attempt = 0;
        loop {
            attempt += 1;
            let before = desktop::windows(&c.guard);
            let frame = capture::grab(request, &before, &monitors)?;
            let after = desktop::windows(&c.guard);
            let stable = capture_set_stable(&frame.source, &before, &after);
            if !stable && attempt < CAPTURE_ATTEMPTS {
                continue;
            }
            let (windows, extra) = if stable {
                (after, Vec::new())
            } else {
                let extra = unstable_masks(&frame.source, &before, &after);
                (union_for_mask(&before, &after), extra)
            };
            return Ok(capture::finish(
                request,
                frame,
                &windows,
                c.capture_password_budget_ms,
                extra,
                |w, left| self.uia_password_rects(c, w.id, left.min(per_window)),
            ));
        }
    }
}
