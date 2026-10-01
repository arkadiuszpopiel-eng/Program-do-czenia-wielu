use std::sync::Mutex;

use super::*;
use crate::keys::{ChordKey, VK_CONTROL, VK_RETURN};

struct Mock {
    now: Mutex<u64>,
    physical: Mutex<Option<u64>>,
    focus: Mutex<TargetWindow>,
    sent: Mutex<Vec<Vec<RawInput>>>,
    touch_after: Option<usize>,
    steal_after: Option<(usize, TargetWindow)>,
}

fn win(id: u64, image: &str) -> TargetWindow {
    TargetWindow {
        id: WindowId(id),
        pid: 100 + u32::try_from(id).unwrap(),
        image: image.into(),
        elevated: false,
    }
}

impl Mock {
    fn new() -> Self {
        Self {
            now: Mutex::new(10_000),
            physical: Mutex::new(Some(1_000)),
            focus: Mutex::new(win(1, "notepad.exe")),
            sent: Mutex::new(Vec::new()),
            touch_after: None,
            steal_after: None,
        }
    }
}

impl InputBackend for Mock {
    fn now_ms(&self) -> u64 {
        *self.now.lock().unwrap()
    }
    fn pause_ms(&self, ms: u64) {
        *self.now.lock().unwrap() += ms;
    }
    fn last_physical_input_ms(&self) -> Option<u64> {
        *self.physical.lock().unwrap()
    }
    fn foreground_target(&self) -> Option<TargetWindow> {
        Some(self.focus.lock().unwrap().clone())
    }
    fn target_at(&self, x: i32, _y: i32) -> Option<TargetWindow> {
        (x >= 0).then(|| self.focus.lock().unwrap().clone())
    }
    fn inject(&self, events: &[RawInput]) -> Result<(), GuiError> {
        let mut sent = self.sent.lock().unwrap();
        sent.push(events.to_vec());
        if self.touch_after == Some(sent.len()) {
            *self.physical.lock().unwrap() = Some(self.now_ms());
        }
        if let Some((n, w)) = &self.steal_after
            && *n == sent.len()
        {
            *self.focus.lock().unwrap() = w.clone();
        }
        Ok(())
    }
}

fn text_plan(text: &str) -> InputPlan {
    InputPlan {
        window: WindowId(1),
        steps: vec![InputStep::Text { text: text.into() }],
    }
}

fn run(m: &Mock, plan: &InputPlan) -> Result<InputReport, GuiError> {
    execute(
        plan,
        m,
        &TargetGuard::baseline(),
        &InputPacing::default(),
        &InputControl::new(),
    )
}

#[test]
fn text_is_split_into_balanced_batches() {
    let m = Mock::new();
    let text = "Zażółć gęślą jaźń 😀\nkoniec\tx";
    let r = run(&m, &text_plan(text)).unwrap();
    let sent = m.sent.lock().unwrap();
    assert_eq!(r.batches as usize, sent.len());
    assert!(sent.len() >= 2);
    for batch in sent.iter() {
        let downs = batch
            .iter()
            .filter(|e| {
                matches!(
                    e,
                    RawInput::Unicode { up: false, .. } | RawInput::Key { up: false, .. }
                )
            })
            .count();
        let ups = batch
            .iter()
            .filter(|e| {
                matches!(
                    e,
                    RawInput::Unicode { up: true, .. } | RawInput::Key { up: true, .. }
                )
            })
            .count();
        assert_eq!(downs, ups, "każda paczka zwalnia wszystko, co wcisnęła");
    }
    assert!(sent.concat().contains(&RawInput::Key {
        vk: VK_RETURN,
        up: false
    }));
    assert!(r.elapsed_ms >= InputPacing::default().batch_interval_ms);
}

#[test]
fn chords_clicks_and_policy() {
    let m = Mock::new();
    let plan = InputPlan {
        window: WindowId(1),
        steps: vec![
            InputStep::Keys {
                chord: KeyChord::parse("Ctrl+S").unwrap(),
            },
            InputStep::Click {
                x: 5,
                y: 5,
                button: MouseButton::Left,
                double: true,
            },
            InputStep::Scroll {
                x: 5,
                y: 5,
                notches: -3,
                horizontal: false,
            },
        ],
    };
    run(&m, &plan).unwrap();
    let sent = m.sent.lock().unwrap().clone();
    assert_eq!(
        sent[0].first(),
        Some(&RawInput::Key {
            vk: VK_CONTROL,
            up: false
        })
    );
    assert_eq!(
        sent[0].last(),
        Some(&RawInput::Key {
            vk: VK_CONTROL,
            up: true
        })
    );
    assert_eq!(sent[1].len(), 5);
    assert_eq!(
        sent[2][1],
        RawInput::Wheel {
            delta: -360,
            horizontal: false
        }
    );
    let bad = |steps: Vec<InputStep>| InputPlan {
        window: WindowId(1),
        steps,
    };
    let win_r = InputStep::Keys {
        chord: KeyChord::parse("Win+R").unwrap(),
    };
    for plan in [
        bad(vec![win_r]),
        bad(vec![]),
        bad(vec![InputStep::Text {
            text: "a\u{7}b".into(),
        }]),
        bad(vec![InputStep::Scroll {
            x: 0,
            y: 0,
            notches: 0,
            horizontal: true,
        }]),
        text_plan(&"a".repeat(5_001)),
    ] {
        assert!(
            matches!(run(&Mock::new(), &plan), Err(GuiError::Policy(_))),
            "{plan:?}"
        );
    }
    assert_eq!(ChordKey::Letter('S').vk(), 0x53);
}

#[test]
fn protected_or_changed_target_stops_before_injection() {
    let m = Mock::new();
    *m.focus.lock().unwrap() = win(1, r"C:\Alfa\alfa-broker-ui.exe");
    assert!(matches!(
        run(&m, &text_plan("x")),
        Err(GuiError::ProtectedTarget(_))
    ));
    *m.focus.lock().unwrap() = win(2, "notepad.exe");
    assert!(matches!(
        run(&m, &text_plan("x")),
        Err(GuiError::TargetChanged {
            expected: 1,
            actual: Some(2)
        })
    ));
    let mut elevated = win(1, "regedit.exe");
    elevated.elevated = true;
    *m.focus.lock().unwrap() = elevated;
    assert_eq!(run(&m, &text_plan("x")), Err(GuiError::Elevated));
    let click = InputPlan {
        window: WindowId(1),
        steps: vec![InputStep::Click {
            x: -1,
            y: 0,
            button: MouseButton::Right,
            double: false,
        }],
    };
    assert!(matches!(
        run(&Mock::new(), &click),
        Err(GuiError::TargetChanged { actual: None, .. })
    ));
    assert!(m.sent.lock().unwrap().is_empty());
    // Broker-UI wyskakuje na wierzch w trakcie pisania → kolejna paczka nie wychodzi.
    let mut m = Mock::new();
    m.steal_after = Some((1, win(9, "alfa-broker-ui.exe")));
    let long = "x".repeat(100);
    assert!(matches!(
        run(&m, &text_plan(&long)),
        Err(GuiError::ProtectedTarget(_))
    ));
    assert_eq!(m.sent.lock().unwrap().len(), 1);
}

#[test]
fn physical_input_has_priority() {
    let mut m = Mock::new();
    *m.physical.lock().unwrap() = Some(9_900);
    assert_eq!(run(&m, &text_plan("x")), Err(GuiError::UserActive));
    *m.physical.lock().unwrap() = None;
    m.touch_after = Some(2);
    let long = "y".repeat(200);
    assert_eq!(
        run(&m, &text_plan(&long)),
        Err(GuiError::UserInterrupted { sent: 2 })
    );
    assert_eq!(m.sent.lock().unwrap().len(), 2);
    let m = Mock::new();
    let control = InputControl::new();
    control.cancel();
    let r = execute(
        &text_plan("x"),
        &m,
        &TargetGuard::baseline(),
        &InputPacing::default(),
        &control,
    );
    assert_eq!(r, Err(GuiError::Cancelled));
    assert!(control.is_cancelled());
}
