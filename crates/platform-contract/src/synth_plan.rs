//! Podział planu wejścia na atomowe paczki zdarzeń (jedno `SendInput` = cały skrót, klik albo
//! ≤ N jednostek tekstu; każde wciśnięcie ma puszczenie w tej samej paczce) i limity planu.

use crate::gui::GuiError;
use crate::keys::{VK_RETURN, VK_TAB};
use crate::synth::{Aim, InputBatch, InputPacing, InputPlan, InputStep, RawInput};

fn press(events: &mut Vec<RawInput>, vk: u16) {
    events.push(RawInput::Key { vk, up: false });
    events.push(RawInput::Key { vk, up: true });
}

fn text_batches(
    text: &str,
    pacing: &InputPacing,
    out: &mut Vec<InputBatch>,
) -> Result<(), GuiError> {
    let mut current: Vec<RawInput> = Vec::new();
    let mut units = 0usize;
    for c in text.chars() {
        match c {
            '\r' => continue,
            '\n' => press(&mut current, VK_RETURN),
            '\t' => press(&mut current, VK_TAB),
            c if c.is_control() => {
                return Err(GuiError::Policy(format!(
                    "znak sterujący U+{:04X} w tekście",
                    u32::from(c)
                )));
            }
            c => {
                let mut buf = [0u16; 2];
                for unit in c.encode_utf16(&mut buf).iter() {
                    current.push(RawInput::Unicode {
                        unit: *unit,
                        up: false,
                    });
                    current.push(RawInput::Unicode {
                        unit: *unit,
                        up: true,
                    });
                }
            }
        }
        units += c.len_utf16();
        if units >= pacing.text_batch_units.max(1) {
            out.push(InputBatch {
                aim: Aim::Focus,
                events: std::mem::take(&mut current),
            });
            units = 0;
        }
    }
    if !current.is_empty() {
        out.push(InputBatch {
            aim: Aim::Focus,
            events: current,
        });
    }
    Ok(())
}

/// Dzieli plan na atomowe paczki i sprawdza limity oraz zasady (skróty systemowe).
pub fn plan_batches(plan: &InputPlan, pacing: &InputPacing) -> Result<Vec<InputBatch>, GuiError> {
    if plan.steps.is_empty() || plan.steps.len() > pacing.max_steps {
        return Err(GuiError::Policy(format!(
            "plan musi mieć 1–{} kroków",
            pacing.max_steps
        )));
    }
    let chars: usize = plan
        .steps
        .iter()
        .map(|s| match s {
            InputStep::Text { text } => text.chars().count(),
            _ => 0,
        })
        .sum();
    if chars > pacing.max_text_chars {
        return Err(GuiError::Policy(format!(
            "tekst dłuższy niż {} znaków",
            pacing.max_text_chars
        )));
    }
    let mut out = Vec::new();
    for step in &plan.steps {
        match step {
            InputStep::Text { text } => text_batches(text, pacing, &mut out)?,
            InputStep::Keys { chord } => {
                if let Some(why) = chord.system_scope() {
                    return Err(GuiError::Policy(format!("skrót {chord}: {why}")));
                }
                let (mods, key) = chord.vks();
                let mut ev: Vec<RawInput> = mods
                    .iter()
                    .map(|&vk| RawInput::Key { vk, up: false })
                    .collect();
                press(&mut ev, key);
                ev.extend(mods.iter().rev().map(|&vk| RawInput::Key { vk, up: true }));
                out.push(InputBatch {
                    aim: Aim::Focus,
                    events: ev,
                });
            }
            InputStep::Click {
                x,
                y,
                button,
                double,
            } => {
                let mut ev = vec![RawInput::MoveTo { x: *x, y: *y }];
                for _ in 0..if *double { 2 } else { 1 } {
                    ev.push(RawInput::Button {
                        button: *button,
                        up: false,
                    });
                    ev.push(RawInput::Button {
                        button: *button,
                        up: true,
                    });
                }
                out.push(InputBatch {
                    aim: Aim::Point { x: *x, y: *y },
                    events: ev,
                });
            }
            InputStep::Scroll {
                x,
                y,
                notches,
                horizontal,
            } => {
                if *notches == 0 || notches.abs() > 20 {
                    return Err(GuiError::Policy("przewinięcie: 1–20 ząbków".into()));
                }
                let ev = vec![
                    RawInput::MoveTo { x: *x, y: *y },
                    RawInput::Wheel {
                        delta: notches * 120,
                        horizontal: *horizontal,
                    },
                ];
                out.push(InputBatch {
                    aim: Aim::Point { x: *x, y: *y },
                    events: ev,
                });
            }
        }
    }
    Ok(out)
}
