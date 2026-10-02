//! Atrapa `voice-dictation`: automat z kontraktu + wirtualne okna w pamięci ([`FakeWindows`]).

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

use std::sync::{Arc, Mutex, MutexGuard};

use platform_contract::{TargetGuard, WindowId, image_file_name};
use voice_dictation_contract::contract_tests::DesktopDriver;
use voice_dictation_contract::{
    Dictation, DictationAction, DictationCfg, DictationError, DictationEvent, DictationMachine,
    DictationMode, DictationStatus, DictationTarget, RefuseReason, StopReason, is_terminal_image,
};

#[derive(Debug, Clone)]
struct Win {
    id: u64,
    image: String,
    elevated: bool,
    password: bool,
    text: String,
}

#[derive(Debug, Default)]
struct Desk {
    windows: Vec<Win>,
    focus: Option<u64>,
}

/// Wirtualne okna (klon dzieli stan).
#[derive(Debug, Clone, Default)]
pub struct FakeWindows(Arc<Mutex<Desk>>);

impl FakeWindows {
    fn lock(&self) -> MutexGuard<'_, Desk> {
        self.0.lock().unwrap_or_else(|p| p.into_inner())
    }

    fn add(&self, image: &str, elevated: bool, password: bool) -> u64 {
        let mut d = self.lock();
        let id = d.windows.len() as u64 + 1;
        d.windows.push(Win {
            id,
            image: image.into(),
            elevated,
            password,
            text: String::new(),
        });
        d.focus = Some(id);
        id
    }
}

impl DesktopDriver for FakeWindows {
    fn open_app(&self, image: &str) -> u64 {
        self.add(image, false, false)
    }
    fn open_password_app(&self) -> u64 {
        self.add("bank.exe", false, true)
    }
    fn open_elevated_app(&self) -> u64 {
        self.add("regedit.exe", true, false)
    }
    fn focus(&self, window: u64) {
        self.lock().focus = Some(window);
    }
    fn text(&self, window: u64) -> String {
        self.lock()
            .windows
            .iter()
            .find(|w| w.id == window)
            .map(|w| w.text.clone())
            .unwrap_or_default()
    }
}

/// Atrapa dyktowania.
#[derive(Debug)]
pub struct FakeDictation {
    machine: DictationMachine,
    windows: FakeWindows,
    guard: TargetGuard,
}

impl FakeDictation {
    /// Atrapa na wirtualnych oknach.
    pub fn new(windows: FakeWindows) -> Self {
        Self {
            machine: DictationMachine::new(DictationCfg::default()),
            windows,
            guard: TargetGuard::baseline(),
        }
    }

    fn apply(&mut self, actions: Vec<DictationAction>, now: u64) {
        for a in actions {
            let focus = self.windows.lock().focus;
            let Some(target) = self.machine.target().map(|t| t.window.0) else {
                return;
            };
            if focus != Some(target) {
                self.machine.foreground(focus.map(WindowId));
                return;
            }
            let mut d = self.windows.lock();
            let Some(w) = d.windows.iter_mut().find(|w| w.id == target) else {
                return;
            };
            match a {
                DictationAction::Type { id, text } => {
                    w.text.push_str(&text);
                    drop(d);
                    self.machine.typed(id, text.chars().count(), now);
                }
                DictationAction::Erase { id, chars } => {
                    for _ in 0..chars {
                        w.text.pop();
                    }
                    drop(d);
                    self.machine.erased(id, chars);
                }
            }
        }
    }
}

impl Dictation for FakeDictation {
    fn start(
        &mut self,
        mode: DictationMode,
        _now_ms: u64,
    ) -> Result<DictationStatus, DictationError> {
        let d = self.windows.lock();
        let w = d
            .focus
            .and_then(|f| d.windows.iter().find(|w| w.id == f).cloned());
        drop(d);
        let reason = match &w {
            None => Some(RefuseReason::NoForeground),
            Some(w) if self.guard.is_protected(u32::MAX, &w.image) => {
                Some(RefuseReason::ProtectedTarget)
            }
            Some(w) if w.elevated => Some(RefuseReason::ElevatedTarget),
            Some(w) if w.password => Some(RefuseReason::PasswordField),
            Some(_) => None,
        };
        if let Some(r) = reason {
            self.machine.refuse(r);
            return Err(DictationError::Refused(r));
        }
        let Some(w) = w else {
            return Err(DictationError::Refused(RefuseReason::NoForeground));
        };
        self.machine.start(
            DictationTarget {
                window: WindowId(w.id),
                pid: u32::MAX,
                app: image_file_name(&w.image),
                terminal: is_terminal_image(&w.image),
            },
            mode,
        );
        Ok(self.machine.status())
    }

    fn stop(&mut self) -> DictationStatus {
        self.machine.stop(StopReason::User);
        self.machine.status()
    }

    fn on_final(&mut self, text: &str, now_ms: u64) -> Result<DictationStatus, DictationError> {
        let actions = self.machine.final_text(text, now_ms)?;
        self.apply(actions, now_ms);
        Ok(self.machine.status())
    }

    fn tick(&mut self, now_ms: u64) -> DictationStatus {
        let focus = self.windows.lock().focus;
        let actions = self.machine.foreground(focus.map(WindowId));
        self.apply(actions, now_ms);
        self.machine.status()
    }

    fn undo_last(&mut self, now_ms: u64) -> Result<DictationStatus, DictationError> {
        let actions = self.machine.undo(now_ms);
        self.apply(actions, now_ms);
        Ok(self.machine.status())
    }

    fn status(&self) -> DictationStatus {
        self.machine.status()
    }

    fn take_events(&mut self) -> Vec<DictationEvent> {
        self.machine.take_events()
    }
}
