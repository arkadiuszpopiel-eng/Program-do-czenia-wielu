//! `BrokerUi` na natywnym oknie (port [`ApprovalSurfacePort`]): kolejka kart (najstarsza na
//! ekranie), stan pierwszego planu okna, odrzucenia z komunikatem w wierszu stanu.

use std::collections::VecDeque;
use std::sync::Arc;

use broker_ui_contract::{
    BrokerUi, CardOptions, HelloPort, UiConfig, UiDecision, UiError, UiEvent, UiStatus,
};
use platform_contract::{ApprovalSurfacePort, SurfaceEvent};
use safety_broker_contract::{ApprovalChallenge, ApprovalId};

use crate::session::{CardSession, Step};

/// Broker-UI na natywnym oknie.
pub struct NativeBrokerUi<S: ApprovalSurfacePort> {
    surface: Arc<S>,
    hello: Arc<dyn HelloPort>,
    opts: CardOptions,
    queue: VecDeque<CardSession>,
    presented: Option<ApprovalId>,
    window_active: bool,
    blocked: Option<String>,
    events: Vec<UiEvent>,
}

impl<S: ApprovalSurfacePort> NativeBrokerUi<S> {
    /// Nowy Broker-UI; `hello` używane tylko przy `hello_enabled`.
    pub fn new(surface: Arc<S>, hello: Arc<dyn HelloPort>, config: UiConfig) -> Self {
        let hello: Arc<dyn HelloPort> = if config.hello_enabled {
            hello
        } else {
            Arc::new(broker_ui_contract::NoHello)
        };
        Self {
            surface,
            hello,
            opts: CardOptions {
                grant_ms: config.grant_ms(),
            },
            queue: VecDeque::new(),
            presented: None,
            window_active: false,
            blocked: None,
            events: Vec::new(),
        }
    }

    fn withdraw_expired(&mut self, now_ms: u64) {
        let expired: Vec<ApprovalId> = self
            .queue
            .iter()
            .filter(|s| s.expires_at_ms() <= now_ms)
            .map(CardSession::id)
            .collect();
        for id in expired {
            self.withdraw(id);
        }
    }

    fn present_front(&mut self, now_ms: u64) -> Result<(), UiError> {
        let Some(front) = self.queue.front_mut() else {
            return Ok(());
        };
        if self.presented == Some(front.id()) {
            return Ok(());
        }
        front.mark_shown(now_ms, self.window_active);
        let id = front.id();
        let view = front.view(now_ms);
        self.surface
            .present(&view)
            .map_err(|e| UiError::Surface(e.to_string()))?;
        self.presented = Some(id);
        self.events.push(UiEvent::Shown { id });
        Ok(())
    }

    fn track_window(&mut self, event: &SurfaceEvent) {
        match event {
            SurfaceEvent::Activated { .. } => self.window_active = true,
            SurfaceEvent::Deactivated { .. } | SurfaceEvent::Closed => self.window_active = false,
            _ => {}
        }
        if matches!(event, SurfaceEvent::Closed) {
            // Okno zniknęło bez decyzji — przy następnym kroku karta wróci na ekran.
            self.presented = None;
        }
    }

    fn finish(&mut self, step: Step, now_ms: u64) -> Option<UiDecision> {
        let id = self.queue.front().map(CardSession::id)?;
        match step {
            Step::Idle => None,
            Step::Hello { verified, then } => {
                self.events.push(UiEvent::HelloUsed { id, verified });
                self.finish(*then, now_ms)
            }
            Step::Rejected(reason) => {
                self.events.push(UiEvent::InputRejected { id, reason });
                if let Some(front) = self.queue.front()
                    && let Err(e) = self.surface.present(&front.view(now_ms))
                {
                    self.blocked = Some(e.to_string());
                }
                None
            }
            Step::Decided(decision) => {
                self.events.push(UiEvent::Decided {
                    id,
                    decision: decision.decision.clone(),
                });
                self.queue.pop_front();
                self.presented = None;
                if self.queue.is_empty() {
                    let _ = self.surface.dismiss();
                }
                Some(decision)
            }
        }
    }
}

impl<S: ApprovalSurfacePort> BrokerUi for NativeBrokerUi<S> {
    fn show(&mut self, challenge: ApprovalChallenge, now_ms: u64) -> Result<(), UiError> {
        if challenge.request.expires_at_ms <= now_ms {
            return Err(UiError::InvalidChallenge("prośba już wygasła".into()));
        }
        if self.queue.iter().any(|s| s.id() == challenge.request.id) {
            return Ok(());
        }
        self.queue
            .push_back(CardSession::new(&challenge, now_ms, self.opts));
        Ok(())
    }

    fn poll_decision(&mut self, now_ms: u64, wait_ms: u32) -> Option<UiDecision> {
        self.withdraw_expired(now_ms);
        if let Err(e) = self.present_front(now_ms) {
            self.blocked = Some(e.to_string());
            return None;
        }
        self.blocked = None;
        // Pierwsze zdarzenie z czekaniem, kolejne już oczekujące — bez czekania.
        let mut wait = wait_ms;
        while let Some(event) = self.surface.next_event(wait) {
            wait = 0;
            self.track_window(&event);
            let hello = self.hello.clone();
            let step = self.queue.front_mut()?.on_event(event, hello.as_ref());
            if let Some(decision) = self.finish(step, now_ms) {
                return Some(decision);
            }
            // Karta zniknęła z ekranu (zamknięte okno) — wróci w następnym kroku.
            self.presented?;
        }
        None
    }

    fn withdraw(&mut self, id: ApprovalId) {
        let before = self.queue.len();
        self.queue.retain(|s| s.id() != id);
        if self.queue.len() == before {
            return;
        }
        self.events.push(UiEvent::Withdrawn { id });
        if self.presented == Some(id) {
            self.presented = None;
            if self.queue.is_empty() {
                let _ = self.surface.dismiss();
            }
        }
    }

    fn queued(&self) -> Vec<ApprovalId> {
        self.queue.iter().map(CardSession::id).collect()
    }

    fn status(&self) -> UiStatus {
        if let Some(reason) = &self.blocked {
            return UiStatus::Blocked {
                reason: reason.clone(),
            };
        }
        match self.queue.len() {
            0 => UiStatus::Hidden,
            n => UiStatus::Pending {
                count: u8::try_from(n).unwrap_or(u8::MAX),
            },
        }
    }

    fn drain_events(&mut self) -> Vec<UiEvent> {
        std::mem::take(&mut self.events)
    }
}
