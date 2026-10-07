//! Panel „Ekran": co agentka robi (akcje GUI bez treści wpisywanej), kto steruje (wskaźnik
//! w pasku tytułu), ostatni zrzut — piksele **wyłącznie w pamięci procesu** (do `gui_screenshot`;
//! nigdy w zdarzeniach, logach ani na dysku) — oraz przejęcie sterowania przez właściciela:
//! „Zatrzymaj sterowanie" anuluje trwające wywołania GUI i wstrzymuje kolejne, dopóki właściciel
//! nie odda sterowania. Każde narzędzie GUI agentki przechodzi przez [`WatchedTool`].

use std::collections::{BTreeMap, VecDeque};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

use app_api::EventHub;
use app_api::dto::{
    AlfaEvent, GuiAction, GuiActionStatus, GuiControl, GuiScreenshot, GuiShotInfo, GuiStatus,
    LocalizedText, iso,
};
use async_trait::async_trait;
use chrono::Utc;
use core_bus_contract::SessionId;
use tokio_util::sync::CancellationToken;
use tools_common_contract::{Tool, ToolCtx, ToolManifest, ToolOutcome};

use crate::describe::{
    action, agent_of, shot_info, status_of, summary, taken_over_outcome, target_of,
};

/// Ile ostatnich akcji pokazuje panel.
pub const MAX_ACTIONS: usize = 20;
/// Jak długo po ostatniej akcji agentka „steruje" (wskaźnik w pasku tytułu).
pub const CONTROL_LINGER: Duration = Duration::from_secs(8);
/// Narzędzie zrzutu (obraz trafia do panelu „Ekran").
pub(crate) const SCREEN_TOOL: &str = "screen_capture";

struct Active {
    control: GuiControl,
    session: SessionId,
    cancel: CancellationToken,
}

struct Shot {
    info: GuiShotInfo,
    png_b64: String,
}

#[derive(Default)]
struct State {
    active: BTreeMap<u64, Active>,
    last: Option<(GuiControl, SessionId, Instant)>,
    actions: VecDeque<GuiAction>,
    shot: Option<Shot>,
    taken_over: bool,
}

/// Stan computer use w aplikacji.
pub struct GuiMonitor {
    state: Mutex<State>,
    events: Option<EventHub>,
    available: bool,
    next: AtomicU64,
}

impl std::fmt::Debug for GuiMonitor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let st = self.lock();
        f.debug_struct("GuiMonitor")
            .field("available", &self.available)
            .field("active", &st.active.len())
            .field("actions", &st.actions.len())
            .field("taken_over", &st.taken_over)
            .finish_non_exhaustive()
    }
}

fn reason_unavailable() -> LocalizedText {
    LocalizedText::new(
        "Sterowanie pulpitem działa tylko w Windows.",
        "Desktop control works on Windows only.",
    )
}

impl GuiMonitor {
    /// Monitor; `events` — zdarzenia `GuiActivity` do UI.
    pub fn new(events: Option<EventHub>, available: bool) -> Self {
        Self {
            state: Mutex::new(State::default()),
            events,
            available,
            next: AtomicU64::new(1),
        }
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn status_of(&self, st: &State) -> GuiStatus {
        let control = st
            .active
            .values()
            .next_back()
            .map(|a| a.control.clone())
            .or_else(|| {
                st.last
                    .as_ref()
                    .filter(|(_, _, at)| at.elapsed() < CONTROL_LINGER)
                    .map(|(c, _, _)| c.clone())
            });
        GuiStatus {
            available: self.available,
            reason: (!self.available).then(reason_unavailable),
            control,
            taken_over: st.taken_over,
            actions: st.actions.iter().rev().cloned().collect(),
            screenshot: st.shot.as_ref().map(|s| s.info.clone()),
        }
    }

    /// Stan panelu.
    pub fn status(&self) -> GuiStatus {
        self.status_of(&self.lock())
    }

    /// Ostatni zrzut agentki (zamaskowany w porcie) jako `data:` URL.
    pub fn screenshot(&self) -> Option<GuiScreenshot> {
        self.lock().shot.as_ref().map(|s| GuiScreenshot {
            info: s.info.clone(),
            data_url: format!("data:image/png;base64,{}", s.png_b64),
        })
    }

    fn announce(&self) {
        if let Some(events) = &self.events {
            events.emit(AlfaEvent::GuiActivity {
                status: self.status(),
            });
        }
    }

    /// Po wygaśnięciu „sterowania" — zdarzenie z `control = null` (wskaźnik gaśnie).
    fn announce_later(self: &Arc<Self>) {
        let Ok(handle) = tokio::runtime::Handle::try_current() else {
            return;
        };
        let me = Arc::downgrade(self);
        handle.spawn(async move {
            tokio::time::sleep(CONTROL_LINGER + Duration::from_millis(50)).await;
            if let Some(me) = me.upgrade() {
                let idle = {
                    let st = me.lock();
                    st.active.is_empty()
                        && st
                            .last
                            .as_ref()
                            .is_some_and(|l| l.2.elapsed() >= CONTROL_LINGER)
                };
                if idle {
                    me.announce();
                }
            }
        });
    }

    /// „Zatrzymaj sterowanie": anuluje trwające wywołania GUI, wstrzymuje kolejne (przejęcie);
    /// zwraca sesje, w których agentki sterowały (rdzeń zatrzymuje ich przebiegi).
    pub fn take_over(&self) -> Vec<SessionId> {
        let sessions = {
            let mut st = self.lock();
            st.taken_over = true;
            let mut out: Vec<SessionId> = Vec::new();
            for a in st.active.values() {
                a.cancel.cancel();
                if !out.contains(&a.session) {
                    out.push(a.session.clone());
                }
            }
            if let Some((_, s, at)) = &st.last
                && at.elapsed() < CONTROL_LINGER
                && !out.contains(s)
            {
                out.push(s.clone());
            }
            st.last = None;
            out
        };
        self.announce();
        sessions
    }

    /// „Oddaj sterowanie": agentki mogą znów używać narzędzi GUI.
    pub fn release(&self) {
        self.lock().taken_over = false;
        self.announce();
    }

    /// Czy właściciel przejął sterowanie.
    pub fn taken_over(&self) -> bool {
        self.lock().taken_over
    }

    fn begin(&self, ctx: &ToolCtx, m: &ToolManifest) -> Option<u64> {
        let id = self.next.fetch_add(1, Ordering::Relaxed);
        {
            let mut st = self.lock();
            if st.taken_over {
                return None;
            }
            let control = GuiControl {
                session_id: ctx.holder.session.to_string(),
                agent: agent_of(ctx),
                tool: m.name.clone(),
                since: iso(Utc::now()),
            };
            st.active.insert(
                id,
                Active {
                    control,
                    session: ctx.holder.session.clone(),
                    cancel: ctx.cancel.clone(),
                },
            );
            push_action(&mut st, action(id, ctx, m, GuiActionStatus::Running, None));
        }
        self.announce();
        Some(id)
    }

    fn finish(
        self: &Arc<Self>,
        id: u64,
        ctx: &ToolCtx,
        m: &ToolManifest,
        args: &serde_json::Value,
        out: &ToolOutcome,
        took: Duration,
    ) {
        {
            let mut st = self.lock();
            if let Some(a) = st.active.remove(&id) {
                st.last = Some((a.control, a.session, Instant::now()));
            }
            let status = status_of(&out.status);
            let mut done = action(id, ctx, m, status, Some(took));
            done.target = target_of(&out.data);
            done.summary = summary(m, args, out);
            if let Some(slot) = st.actions.iter_mut().find(|a| a.id == id) {
                *slot = done;
            } else {
                push_action(&mut st, done);
            }
            if m.name == SCREEN_TOOL
                && status == GuiActionStatus::Ok
                && let Some(image) = out.images.first()
            {
                st.shot = Some(Shot {
                    info: shot_info(ctx, &out.data),
                    png_b64: image.data_base64.clone(),
                });
            }
        }
        self.announce();
        self.announce_later();
    }

    /// Opakowuje narzędzia GUI (panel „Ekran", przejęcie sterowania).
    pub fn wrap(self: &Arc<Self>, tools: Vec<Arc<dyn Tool>>) -> Vec<Arc<dyn Tool>> {
        tools
            .into_iter()
            .map(|inner| {
                Arc::new(WatchedTool {
                    inner,
                    monitor: self.clone(),
                }) as Arc<dyn Tool>
            })
            .collect()
    }
}

fn push_action(st: &mut State, a: GuiAction) {
    st.actions.push_back(a);
    while st.actions.len() > MAX_ACTIONS {
        st.actions.pop_front();
    }
}

/// Narzędzie GUI pod nadzorem panelu „Ekran".
pub struct WatchedTool {
    inner: Arc<dyn Tool>,
    monitor: Arc<GuiMonitor>,
}

#[async_trait]
impl Tool for WatchedTool {
    fn manifest(&self) -> &ToolManifest {
        self.inner.manifest()
    }

    async fn call(&self, args: serde_json::Value, ctx: &ToolCtx) -> ToolOutcome {
        let m = self.inner.manifest();
        let Some(id) = self.monitor.begin(ctx, m) else {
            return taken_over_outcome(&m.title.to_lowercase());
        };
        let started = Instant::now();
        let out = self.inner.call(args.clone(), ctx).await;
        self.monitor
            .finish(id, ctx, m, &args, &out, started.elapsed());
        out
    }
}
