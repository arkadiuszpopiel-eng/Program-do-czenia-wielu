//! Wejście syntetyczne na wirtualnym pulpicie: `InputBackend` (zegar, fizyczne wejście, okno
//! z fokusem i pod punktem, atomowe wstrzyknięcie) + `InputPort` przez wspólny `execute_input`
//! z kontraktu. Wstrzyknięcie zapisuje skutek dla okna, które **faktycznie** dostało zdarzenie
//! (klawiatura → okno z fokusem, mysz → okno pod kursorem), i odpala skrypty zdarzeń.

use platform_contract::{
    GuiError, InputBackend, InputControl, InputPlan, InputPort, InputReport, RawInput,
    TargetWindow, WindowId, execute_input,
};

use super::{FakeDesktop, GuiRecordKind, ScriptEvent, State, Win};

const VK_RETURN: u16 = 0x0D;
const VK_TAB: u16 = 0x09;

fn target(w: &Win) -> TargetWindow {
    TargetWindow {
        id: w.info.id,
        pid: w.info.pid,
        image: w.info.image.clone(),
        elevated: w.info.elevated,
    }
}

fn focused(s: &State) -> Option<WindowId> {
    s.windows.iter().find(|w| w.info.focused).map(|w| w.info.id)
}

fn deliver(s: &mut State, event: &RawInput) {
    let window = match event {
        RawInput::Key { .. } | RawInput::Unicode { .. } => focused(s),
        RawInput::MoveTo { x, y } => {
            s.cursor = (*x, *y);
            None
        }
        RawInput::Button { .. } | RawInput::Wheel { .. } => {
            let (x, y) = s.cursor;
            s.top_at(x, y).map(|w| w.info.id)
        }
    };
    let Some(id) = window else {
        return;
    };
    if let Ok(w) = s.win_mut(id) {
        match event {
            RawInput::Unicode { unit, up: false } => w.typed.push(*unit),
            RawInput::Key {
                vk: VK_RETURN,
                up: false,
            } => w.typed.push(0x0A),
            RawInput::Key {
                vk: VK_TAB,
                up: false,
            } => w.typed.push(0x09),
            _ => {}
        }
    }
    s.record(id, GuiRecordKind::Input(format!("{event:?}")));
}

impl InputBackend for FakeDesktop {
    fn now_ms(&self) -> u64 {
        self.lock().clock
    }

    fn pause_ms(&self, ms: u64) {
        self.lock().clock += ms;
    }

    fn last_physical_input_ms(&self) -> Option<u64> {
        self.lock().physical
    }

    fn foreground_target(&self) -> Option<TargetWindow> {
        let s = self.lock();
        s.windows.iter().find(|w| w.info.focused).map(target)
    }

    fn target_at(&self, x: i32, y: i32) -> Option<TargetWindow> {
        self.lock().top_at(x, y).map(target)
    }

    fn inject(&self, events: &[RawInput]) -> Result<(), GuiError> {
        let mut s = self.lock();
        for e in events {
            deliver(&mut s, e);
        }
        s.batches += 1;
        let due: Vec<ScriptEvent> = s
            .script
            .iter()
            .filter(|(n, _)| *n == s.batches)
            .map(|(_, e)| *e)
            .collect();
        for event in due {
            match event {
                ScriptEvent::PhysicalInput => s.physical = Some(s.clock),
                ScriptEvent::Raise(id) => s.raise(id),
            }
        }
        Ok(())
    }
}

impl InputPort for FakeDesktop {
    fn send(&self, plan: &InputPlan, control: &InputControl) -> Result<InputReport, GuiError> {
        execute_input(plan, self, &self.guard, &self.pacing, control)
    }
}
