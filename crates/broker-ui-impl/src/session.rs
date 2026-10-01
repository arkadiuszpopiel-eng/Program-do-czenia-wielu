//! Sesja jednej karty: zdarzenia okna → decyzja z dowodem fizycznego wejścia.
//!
//! **Jedyne miejsce w Broker-UI, gdzie powstaje `PhysicalInputProof`** (`broker_ui_only`) — i tylko
//! po [`check_input`]: wejście niewstrzyknięte, okno nieprzerwanie na pierwszym planie ≥ 500 ms,
//! niezasłonięte, w oknie ważności karty. Nonce wyzwania trafia do dowodu raz: po decyzji sesja
//! jest zamknięta i kolejne zdarzenia niczego nie dają.

use broker_ui_contract::{
    ApprovalCard, CardOptions, ForegroundTracker, HelloOutcome, HelloPort, RejectReason,
    UiDecision, check_input,
};
use platform_contract::{InputSample, SurfaceEvent, SurfaceView};
use safety_broker_contract::{
    ApprovalChallenge, ApprovalDecision, ApprovalId, InputSource, Nonce, broker_ui_only,
};

use crate::view::{BTN_DENY, decision_for, view_of};

/// Wynik obsługi zdarzenia.
#[derive(Debug, PartialEq, Eq)]
pub enum Step {
    /// Nic do zrobienia.
    Idle,
    /// Decyzja z dowodem.
    Decided(UiDecision),
    /// Wejście odrzucone (powód trafia do wiersza stanu).
    Rejected(RejectReason),
    /// Użyto Windows Hello.
    Hello {
        /// Czy potwierdzono.
        verified: bool,
        /// Dalszy wynik.
        then: Box<Step>,
    },
}

/// Sesja karty.
#[derive(Debug)]
pub struct CardSession {
    card: ApprovalCard,
    nonce: Nonce,
    shown_at_ms: Option<u64>,
    fg: ForegroundTracker,
    status: String,
    done: bool,
}

impl CardSession {
    /// Sesja z wyzwania (karta budowana na chwilę `now_ms`).
    pub fn new(challenge: &ApprovalChallenge, now_ms: u64, opts: CardOptions) -> Self {
        Self {
            card: ApprovalCard::from_request(&challenge.request, now_ms, opts),
            nonce: challenge.nonce,
            shown_at_ms: None,
            fg: ForegroundTracker::default(),
            status: String::new(),
            done: false,
        }
    }

    /// Prośba.
    pub fn id(&self) -> ApprovalId {
        self.card.id
    }

    /// Karta.
    pub fn card(&self) -> &ApprovalCard {
        &self.card
    }

    /// Termin.
    pub fn expires_at_ms(&self) -> u64 {
        self.card.expires_at_ms
    }

    /// Karta trafiła na ekran w `at_ms`; jeśli okno już jest aktywne, odliczanie 500 ms startuje
    /// teraz (nowa karta w aktywnym oknie nie przejmie kliknięcia przeznaczonego dla poprzedniej).
    pub fn mark_shown(&mut self, at_ms: u64, window_active: bool) {
        self.shown_at_ms = Some(at_ms);
        self.fg = ForegroundTracker::default();
        if window_active {
            self.fg.activated(at_ms);
        }
    }

    /// Widok na chwilę `now_ms`.
    pub fn view(&self, now_ms: u64) -> SurfaceView {
        view_of(&self.card, &self.status, now_ms)
    }

    /// Obsługa zdarzenia okna.
    pub fn on_event(&mut self, event: SurfaceEvent, hello: &dyn HelloPort) -> Step {
        match event {
            SurfaceEvent::Activated { at_ms } => {
                self.fg.activated(at_ms);
                Step::Idle
            }
            SurfaceEvent::Deactivated { .. } => {
                self.fg.deactivated();
                Step::Idle
            }
            SurfaceEvent::Button {
                id,
                input,
                occluded,
            } => match decision_for(&self.card, id) {
                Some(decision) => self.accept(input, occluded, decision, hello),
                None => Step::Idle,
            },
            SurfaceEvent::Cancel { input } => {
                let deny = decision_for(&self.card, BTN_DENY).unwrap_or(ApprovalDecision::Deny);
                self.accept(input, false, deny, hello)
            }
            SurfaceEvent::Closed => Step::Idle,
        }
    }

    fn accept(
        &mut self,
        input: InputSample,
        occluded: bool,
        decision: ApprovalDecision,
        hello: &dyn HelloPort,
    ) -> Step {
        if self.done {
            return Step::Idle;
        }
        let Some(shown) = self.shown_at_ms else {
            return self.reject(RejectReason::BeforeShown);
        };
        let source = match check_input(&self.fg, &input, occluded, shown, self.card.expires_at_ms) {
            Ok(source) => source,
            Err(reason) => return self.reject(reason),
        };
        let needs_hello = self.card.hello_required && decision != ApprovalDecision::Deny;
        if !needs_hello {
            return self.decide(decision, source, input);
        }
        let prompt = format!("Alfa: potwierdź — {}", self.card.title);
        match hello.verify(&prompt) {
            HelloOutcome::Verified => Step::Hello {
                verified: true,
                then: Box::new(self.decide(decision, InputSource::WindowsHello, input)),
            },
            HelloOutcome::Cancelled => Step::Hello {
                verified: false,
                then: Box::new(self.reject(RejectReason::HelloFailed)),
            },
            HelloOutcome::Unavailable => self.reject(RejectReason::HelloUnavailable),
        }
    }

    fn reject(&mut self, reason: RejectReason) -> Step {
        self.status = format!("Nie przyjęto: {reason}.");
        Step::Rejected(reason)
    }

    fn decide(
        &mut self,
        decision: ApprovalDecision,
        source: InputSource,
        input: InputSample,
    ) -> Step {
        self.done = true;
        let proof = broker_ui_only::physical_input_proof(
            self.card.id,
            self.nonce,
            source,
            input.injected,
            input.at_ms,
        );
        Step::Decided(UiDecision {
            id: self.card.id,
            decision,
            proof,
        })
    }
}
