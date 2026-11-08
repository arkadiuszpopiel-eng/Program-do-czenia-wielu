//! Usługa dyktowania na portach platformy: cel = okno na pierwszym planie w chwili startu
//! (`DesktopPort`), odmowa dla okien chronionych (`TargetGuard`), administratora (UIPI) i pól haseł
//! (UIA, fail-closed), wpisywanie przez `InputPort` w porcjach ≤ 8 jednostek UTF-16 — każda porcja
//! to jedna atomowa paczka `SendInput`, więc po przerwaniu wiadomo dokładnie, ile wpisano
//! (reszta czeka na powrót do okna). Strażnik celów portu sprawdza okno przed każdą paczką.

use std::sync::Arc;

use platform_contract::{
    ChordKey, DesktopPort, GuiError, InputControl, InputPlan, InputPort, InputStep, KeyChord,
    UiaPort, image_file_name,
};
use voice_dictation_contract::{
    Dictation, DictationAction, DictationCfg, DictationError, DictationEvent, DictationMachine,
    DictationMode, DictationPhase, DictationStatus, DictationTarget, PauseReason, RefuseReason,
    StopReason, is_terminal_image,
};

use crate::focus::password_risk;

/// Jednostek UTF-16 na porcję (≤ najmniejsza paczka portu — porcja nigdy nie dzieli się na dwie).
pub const CHUNK_UNITS: usize = 8;
/// Backspace na plan (limit kroków planu portu).
const ERASE_STEPS: usize = 32;
/// Ile ms po wejściu użytkownika wznowić wpisywanie.
const USER_RESUME_MS: u64 = 1_500;

/// Porty dyktowania.
#[derive(Clone)]
pub struct DictationPorts {
    /// Okna (pierwszy plan, strażnik).
    pub desktop: Arc<dyn DesktopPort>,
    /// UIA (pole hasła).
    pub uia: Arc<dyn UiaPort>,
    /// Wejście syntetyczne.
    pub input: Arc<dyn InputPort>,
}

/// Usługa dyktowania.
pub struct DictationService {
    ports: DictationPorts,
    machine: DictationMachine,
    control: InputControl,
    busy_since: Option<u64>,
    last_final_ms: u64,
}

impl std::fmt::Debug for DictationService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DictationService")
            .field("status", &self.machine.status())
            .finish_non_exhaustive()
    }
}

/// Porcje tekstu ≤ [`CHUNK_UNITS`] jednostek UTF-16.
pub fn chunks(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut units = 0;
    for c in text.chars() {
        if units + c.len_utf16() > CHUNK_UNITS {
            out.push(std::mem::take(&mut cur));
            units = 0;
        }
        cur.push(c);
        units += c.len_utf16();
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

impl DictationService {
    /// Usługa z konfiguracją.
    pub fn new(ports: DictationPorts, cfg: DictationCfg) -> Self {
        Self {
            ports,
            machine: DictationMachine::new(cfg),
            control: InputControl::new(),
            busy_since: None,
            last_final_ms: 0,
        }
    }

    /// Anulowanie (kill-switch, „stop”): przerywa wpisywanie przed najbliższą paczką i kończy sesję.
    pub fn cancel(&mut self) {
        self.control.cancel();
        self.machine.stop(StopReason::Cancelled);
        self.control = InputControl::new();
    }

    fn refuse(&mut self, reason: RefuseReason) -> DictationError {
        self.machine.refuse(reason);
        DictationError::Refused(reason)
    }

    fn stop_refused(&mut self, reason: RefuseReason) {
        self.machine.refuse(reason);
        self.machine.stop(StopReason::Cancelled);
    }

    fn execute(&mut self, actions: Vec<DictationAction>, now: u64) -> Result<(), DictationError> {
        for a in actions {
            match a {
                DictationAction::Type { id, text } => self.type_phrase(id, &text, now)?,
                DictationAction::Erase { id, chars } => self.erase(id, chars, now)?,
            }
            if self.machine.status().phase != DictationPhase::Active {
                break;
            }
        }
        Ok(())
    }

    /// Błąd wejścia → stan automatu. `Ok(())` — przerwano bez błędu sesji.
    fn on_input_error(&mut self, e: GuiError, now: u64) -> Result<(), DictationError> {
        match e {
            GuiError::UserActive | GuiError::UserInterrupted { .. } => {
                self.busy_since = Some(now);
                self.machine.user_busy();
            }
            GuiError::TargetChanged { .. } => {
                self.machine.foreground(None);
            }
            GuiError::ProtectedTarget(_) => self.stop_refused(RefuseReason::ProtectedTarget),
            GuiError::Elevated => self.stop_refused(RefuseReason::ElevatedTarget),
            GuiError::Cancelled => self.machine.stop(StopReason::Cancelled),
            other => return Err(DictationError::Platform(other.to_string())),
        }
        Ok(())
    }

    fn type_phrase(&mut self, id: u64, text: &str, now: u64) -> Result<(), DictationError> {
        let Some(target) = self.machine.target().cloned() else {
            return Ok(());
        };
        if password_risk(self.ports.uia.as_ref(), target.window) {
            // Fokus przeszedł do pola hasła (albo nie da się tego wykluczyć) — nic nie wpisujemy.
            self.stop_refused(RefuseReason::PasswordField);
            return Ok(());
        }
        let mut typed = 0;
        for chunk in chunks(text) {
            let plan = InputPlan {
                window: target.window,
                steps: vec![InputStep::Text {
                    text: chunk.clone(),
                }],
            };
            if let Err(e) = self.ports.input.send(&plan, &self.control) {
                self.machine.typed(id, typed, now);
                return self.on_input_error(e, now);
            }
            typed += chunk.chars().count();
        }
        self.machine.typed(id, typed, now);
        Ok(())
    }

    fn erase(&mut self, id: u64, chars: usize, now: u64) -> Result<(), DictationError> {
        let Some(target) = self.machine.target().cloned() else {
            return Ok(());
        };
        let backspace = KeyChord {
            ctrl: false,
            alt: false,
            shift: false,
            win: false,
            key: ChordKey::Backspace,
        };
        let mut erased = 0;
        while erased < chars {
            let n = (chars - erased).min(ERASE_STEPS);
            let plan = InputPlan {
                window: target.window,
                steps: vec![InputStep::Keys { chord: backspace }; n],
            };
            if let Err(e) = self.ports.input.send(&plan, &self.control) {
                // Plan Backspace to n paczek — nie wiemy, ile przeszło; cofnięcie uznajemy za
                // częściowe (historia czyszczona przy pauzie).
                self.machine.erased(id, erased);
                return self.on_input_error(e, now);
            }
            erased += n;
        }
        self.machine.erased(id, erased);
        Ok(())
    }
}

impl Dictation for DictationService {
    fn start(
        &mut self,
        mode: DictationMode,
        now_ms: u64,
    ) -> Result<DictationStatus, DictationError> {
        let fg = self
            .ports
            .desktop
            .foreground()
            .map_err(|e| DictationError::Platform(e.to_string()))?;
        let Some(w) = fg else {
            return Err(self.refuse(RefuseReason::NoForeground));
        };
        if w.protected || self.ports.desktop.guard().is_protected(w.pid, &w.image) {
            return Err(self.refuse(RefuseReason::ProtectedTarget));
        }
        if w.elevated {
            return Err(self.refuse(RefuseReason::ElevatedTarget));
        }
        if password_risk(self.ports.uia.as_ref(), w.id) {
            return Err(self.refuse(RefuseReason::PasswordField));
        }
        self.machine.start(
            DictationTarget {
                window: w.id,
                pid: w.pid,
                app: image_file_name(&w.image),
                terminal: is_terminal_image(&w.image),
            },
            mode,
        );
        self.last_final_ms = now_ms;
        Ok(self.machine.status())
    }

    fn stop(&mut self) -> DictationStatus {
        self.machine.stop(StopReason::User);
        self.machine.status()
    }

    fn on_final(&mut self, text: &str, now_ms: u64) -> Result<DictationStatus, DictationError> {
        let actions = self.machine.final_text(text, now_ms)?;
        self.last_final_ms = now_ms;
        self.execute(actions, now_ms)?;
        Ok(self.machine.status())
    }

    fn tick(&mut self, now_ms: u64) -> DictationStatus {
        if self.machine.status().phase == DictationPhase::Idle {
            return self.machine.status();
        }
        let fg = self.ports.desktop.foreground().ok().flatten();
        let target = self.machine.target().map(|t| t.window);
        if fg.is_none() && target.is_some_and(|t| self.ports.desktop.window(t).is_err()) {
            self.machine.stop(StopReason::TargetGone);
            return self.machine.status();
        }
        let mut actions = self.machine.foreground(fg.map(|w| w.id));
        let user_paused =
            self.machine.status().phase == DictationPhase::Paused(PauseReason::UserTyping);
        if user_paused
            && self
                .busy_since
                .is_some_and(|t| now_ms >= t + USER_RESUME_MS)
        {
            self.busy_since = None;
            actions = self.machine.resume_after_user();
        }
        let idle = self.machine.cfg().idle_stop_ms;
        if self.machine.status().mode == DictationMode::Toggle
            && idle > 0
            && now_ms.saturating_sub(self.last_final_ms) >= idle
        {
            self.machine.stop(StopReason::Idle);
            return self.machine.status();
        }
        // Błąd platformy przy ponowieniu — sesja trwa, zostaje w zdarzeniach jako pauza.
        let _ = self.execute(actions, now_ms);
        self.machine.status()
    }

    fn undo_last(&mut self, now_ms: u64) -> Result<DictationStatus, DictationError> {
        if self.machine.status().phase == DictationPhase::Idle {
            return Err(DictationError::NotActive);
        }
        let actions = self.machine.undo(now_ms);
        self.execute(actions, now_ms)?;
        Ok(self.machine.status())
    }

    fn status(&self) -> DictationStatus {
        self.machine.status()
    }

    fn take_events(&mut self) -> Vec<DictationEvent> {
        self.machine.take_events()
    }
}
