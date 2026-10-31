//! Karta → widok natywnego okna: polski tekst, duże przyciski, „Odmów” pierwsza i z fokusem,
//! kolor ryzyka zawsze z ikoną i tekstem; przy wysokim ryzyku okno może przejąć fokus
//! (alertdialog), inaczej pokazuje się bez kradzieży fokusu.

use broker_ui_contract::{ApprovalCard, DecisionOption, JUDGE_HELP, mode_name_pl};
use platform_contract::{SurfaceButton, SurfaceChoice, SurfaceTone, SurfaceView};
use risk_classifier_contract::RiskLevel;
use safety_broker_contract::{ApprovalDecision, UNTIL_REVOKED};

/// Przycisk „Odmów” (fokus startowy; `Esc` działa tak samo).
pub const BTN_DENY: u16 = 100;
/// Przycisk „Zezwól tylko teraz” (przy zmianie trybu: „Włącz: <tryb>”).
pub const BTN_ONCE: u16 = 101;
/// Pierwszy przycisk „Zawsze w tym zakresie” (kolejne czasy: 103, 104, …, ostatni „Do odwołania”).
pub const BTN_SCOPED: u16 = 102;

/// Przyciski opcji karty: „Odmów”, „Zezwól tylko teraz”, potem czasy zgody po kolei.
fn buttons_of(card: &ApprovalCard) -> impl Iterator<Item = (u16, &DecisionOption)> {
    let mut scoped = BTN_SCOPED;
    card.options.iter().map(move |o| {
        let id = match o {
            DecisionOption::Deny => BTN_DENY,
            DecisionOption::AllowOnce => BTN_ONCE,
            DecisionOption::AllowInScope { .. } => {
                scoped += 1;
                scoped - 1
            }
        };
        (id, o)
    })
}

/// Czas zgody na przycisku: „1 h”, „24 h” albo „Do odwołania”.
fn grant_label(until_ms: u64, now_ms: u64) -> String {
    if until_ms == UNTIL_REVOKED {
        return "Do odwołania".to_owned();
    }
    let hours = until_ms.saturating_sub(now_ms).div_ceil(3_600_000);
    format!("{hours} h")
}

/// Decyzja odpowiadająca przyciskowi (tylko opcje obecne na karcie).
pub fn decision_for(card: &ApprovalCard, button: u16) -> Option<ApprovalDecision> {
    buttons_of(card)
        .find(|(id, _)| *id == button)
        .map(|(_, o)| match o {
            DecisionOption::Deny => ApprovalDecision::Deny,
            DecisionOption::AllowOnce => ApprovalDecision::Allow,
            DecisionOption::AllowInScope { scope, until_ms } => ApprovalDecision::AllowInScope {
                scope: scope.clone(),
                until_ms: *until_ms,
            },
        })
}

fn tone(risk: RiskLevel) -> SurfaceTone {
    match risk {
        RiskLevel::Low => SurfaceTone::Low,
        RiskLevel::Medium => SurfaceTone::Medium,
        RiskLevel::High => SurfaceTone::High,
        RiskLevel::Critical => SurfaceTone::Critical,
    }
}

/// Widok karty z wierszem stanu; zakres „zawsze” w godzinach od chwili `now_ms`.
pub fn view_of(card: &ApprovalCard, status: &str, now_ms: u64) -> SurfaceView {
    // Plakietki oddzielone „ · ”: pierwsza w kolorze ryzyka (ikonę rysuje okno według tonu).
    let mut badge = format!("Ryzyko {}", card.risk.label_pl());
    if card.mode_change.is_some() {
        badge.push_str(" · zmiana trybu pracy");
    }
    if card.voice_origin {
        badge.push_str(" · zlecone głosem");
    }
    if card.tainted {
        badge.push_str(" · niezaufana treść w sesji");
    }
    let details = card.details.clone();
    let hello = if card.hello_required {
        " (Windows Hello)"
    } else {
        ""
    };
    let buttons: Vec<SurfaceButton> = buttons_of(card)
        .map(|(id, o)| SurfaceButton {
            id,
            label: match o {
                DecisionOption::Deny => "Odmów (Esc)".to_owned(),
                DecisionOption::AllowOnce => match card.mode_change {
                    Some(mode) => format!("Włącz: {}{hello}", mode_name_pl(mode)),
                    None => format!("Zezwól tylko teraz{hello}"),
                },
                DecisionOption::AllowInScope { until_ms, .. } => grant_label(*until_ms, now_ms),
            },
        })
        .collect();
    // Czas zgody wybiera właściciel: rząd „Zawsze w tym zakresie: 1 h · 3 h · 24 h · Do odwołania”.
    let scoped: Vec<u16> = buttons
        .iter()
        .map(|b| b.id)
        .filter(|id| *id >= BTN_SCOPED)
        .collect();
    let choice = (!scoped.is_empty()).then(|| SurfaceChoice {
        label: format!("Zawsze w tym zakresie{hello}:"),
        buttons: scoped,
    });
    let status = if status.is_empty() {
        "Tab — następny przycisk, Spacja — naciśnij, Enter niczego nie zatwierdza, F1 — pomoc."
            .to_owned()
    } else {
        status.to_owned()
    };
    SurfaceView {
        title: card.title.clone(),
        badge,
        tone: tone(card.risk),
        details,
        status,
        buttons,
        initial_focus: BTN_DENY,
        take_focus: card.risk >= RiskLevel::High,
        help: JUDGE_HELP.to_owned(),
        choice,
    }
}
