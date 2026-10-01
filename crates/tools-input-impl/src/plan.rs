//! Argumenty narzędzi → żądanie → plan wejścia. Punkty zawsze w obrębie okna celu (względem jego
//! lewego górnego rogu albo środek elementu UIA tego samego okna); skróty systemowe odrzucane
//! jako zasady (odmowa), błędy składni jako niepoprawne argumenty.

use platform_contract::{
    DesktopWindow, ElementRef, GuiError, InputPlan, InputStep, KeyChord, MouseButton, UiaPort,
    WindowId,
};
use tools_common_contract::{DenialReason, ToolErrorKind, ToolOutcome, parse_args};
use tools_input_contract::{
    ButtonArg, ClickArgs, InputToolsConfig, KeysArgs, MAX_KEYS, ScrollArgs, TypeArgs, check_click,
};
use tools_window_contract::gui::Step;

/// Gdzie trafia wejście.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Aim {
    /// Okno z fokusem (klawiatura).
    Focus,
    /// Punkt ekranu (mysz).
    Point(i32, i32),
}

impl Aim {
    pub(crate) fn point(self) -> Option<(i32, i32)> {
        match self {
            Self::Focus => None,
            Self::Point(x, y) => Some((x, y)),
        }
    }
}

/// Punkt kliknięcia.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ClickAt {
    /// Względem okna.
    Relative(i32, i32),
    /// Środek elementu.
    Element(ElementRef),
}

/// Żądanie po walidacji argumentów.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Request {
    Type {
        window: WindowId,
        text: String,
    },
    Keys {
        window: WindowId,
        chords: Vec<KeyChord>,
    },
    Click {
        window: WindowId,
        at: ClickAt,
        button: MouseButton,
        double: bool,
    },
    Scroll {
        window: WindowId,
        at: Option<(i32, i32)>,
        notches: i32,
        horizontal: bool,
    },
}

impl Request {
    pub(crate) fn window(&self) -> WindowId {
        match self {
            Self::Type { window, .. }
            | Self::Keys { window, .. }
            | Self::Click { window, .. }
            | Self::Scroll { window, .. } => *window,
        }
    }
}

fn invalid(text: &str) -> Box<ToolOutcome> {
    Box::new(ToolOutcome::failed(
        ToolErrorKind::InvalidArgs,
        format!("Niepoprawne argumenty: {text}."),
    ))
}

fn policy(action: &str, why: &str) -> Box<ToolOutcome> {
    let mut o = ToolOutcome::denied(DenialReason::Policy, action);
    o.text = format!("Odmowa: {action} — {why}. Nie ponawiaj tej samej akcji.");
    Box::new(o)
}

/// Argumenty → żądanie.
pub(crate) fn parse(
    tool: &str,
    args: serde_json::Value,
    config: &InputToolsConfig,
) -> Step<Request> {
    match tool {
        "input_type_text" => {
            let a: TypeArgs = parse_args(args)?;
            if a.text.is_empty() {
                return Err(invalid("pusty tekst"));
            }
            if a.text.chars().count() > config.max_text_chars {
                return Err(policy(
                    "wpisanie tekstu",
                    &format!("tekst dłuższy niż {} znaków", config.max_text_chars),
                ));
            }
            Ok(Request::Type {
                window: WindowId(a.window),
                text: a.text,
            })
        }
        "input_keys" => {
            let a: KeysArgs = parse_args(args)?;
            if a.keys.is_empty() || a.keys.len() > MAX_KEYS {
                return Err(invalid(&format!("podaj 1–{MAX_KEYS} skrótów")));
            }
            let mut chords = Vec::new();
            for k in &a.keys {
                let chord = KeyChord::parse(k).map_err(|e| invalid(&e.to_string()))?;
                if let Some(why) = chord.system_scope() {
                    return Err(policy("skrót klawiszowy", &format!("{k}: {why}")));
                }
                chords.push(chord);
            }
            Ok(Request::Keys {
                window: WindowId(a.window),
                chords,
            })
        }
        "input_click" => {
            let a: ClickArgs = parse_args(args)?;
            check_click(&a).map_err(|e| invalid(&e))?;
            let at = match (&a.element, a.x, a.y) {
                (Some(e), _, _) => {
                    ClickAt::Element(ElementRef::parse(e).map_err(|e| invalid(&e.to_string()))?)
                }
                (None, Some(x), Some(y)) => ClickAt::Relative(x, y),
                _ => return Err(invalid("podaj `x` i `y` albo `element`")),
            };
            let button = match a.button.unwrap_or(ButtonArg::Left) {
                ButtonArg::Left => MouseButton::Left,
                ButtonArg::Right => MouseButton::Right,
                ButtonArg::Middle => MouseButton::Middle,
            };
            Ok(Request::Click {
                window: WindowId(a.window),
                at,
                button,
                double: a.double.unwrap_or(false),
            })
        }
        _ => {
            let a: ScrollArgs = parse_args(args)?;
            if a.notches == 0 || a.notches.abs() > 20 {
                return Err(invalid("`notches`: ±1–±20"));
            }
            let at = match (a.x, a.y) {
                (Some(x), Some(y)) => Some((x, y)),
                (None, None) => None,
                _ => return Err(invalid("podaj oba `x` i `y` albo żadne")),
            };
            Ok(Request::Scroll {
                window: WindowId(a.window),
                at,
                notches: a.notches,
                horizontal: a.horizontal.unwrap_or(false),
            })
        }
    }
}

fn inside(target: &DesktopWindow, x: i32, y: i32) -> Result<(i32, i32), GuiError> {
    let (ax, ay) = (
        target.rect.left.saturating_add(x),
        target.rect.top.saturating_add(y),
    );
    if target.rect.contains(ax, ay) {
        Ok((ax, ay))
    } else {
        Err(GuiError::Policy(format!(
            "punkt ({x},{y}) leży poza oknem ({}×{} px)",
            target.rect.width(),
            target.rect.height()
        )))
    }
}

/// Żądanie → plan (punkt elementu odczytany przez UIA tuż przed wysłaniem).
pub(crate) fn build(
    request: Request,
    target: &DesktopWindow,
    uia: &dyn UiaPort,
) -> Result<(InputPlan, Aim), GuiError> {
    let window = target.id;
    let (steps, aim) = match request {
        Request::Type { text, .. } => (vec![InputStep::Text { text }], Aim::Focus),
        Request::Keys { chords, .. } => (
            chords
                .into_iter()
                .map(|chord| InputStep::Keys { chord })
                .collect(),
            Aim::Focus,
        ),
        Request::Click {
            at, button, double, ..
        } => {
            let (x, y) = match at {
                ClickAt::Relative(x, y) => inside(target, x, y)?,
                ClickAt::Element(el) => {
                    if el.window != window {
                        return Err(GuiError::Policy("element należy do innego okna".into()));
                    }
                    let node = uia.element(&el)?;
                    if node.offscreen || node.rect.is_empty() || !node.enabled {
                        return Err(GuiError::Policy(format!(
                            "element „{}” jest niewidoczny albo wyłączony",
                            node.name
                        )));
                    }
                    let (cx, cy) = node.rect.center();
                    inside(target, cx - target.rect.left, cy - target.rect.top)?
                }
            };
            (
                vec![InputStep::Click {
                    x,
                    y,
                    button,
                    double,
                }],
                Aim::Point(x, y),
            )
        }
        Request::Scroll {
            at,
            notches,
            horizontal,
            ..
        } => {
            let (x, y) = match at {
                Some((x, y)) => inside(target, x, y)?,
                None => target.rect.center(),
            };
            (
                vec![InputStep::Scroll {
                    x,
                    y,
                    notches,
                    horizontal,
                }],
                Aim::Point(x, y),
            )
        }
    };
    Ok((InputPlan { window, steps }, aim))
}
