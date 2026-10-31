//! Karta → widok natywnego okna: polski tekst, duże przyciski, „Odmów” pierwsza i z fokusem,
//! kolor ryzyka zawsze z ikoną i tekstem; przy wysokim ryzyku okno może przejąć fokus
//! (alertdialog), inaczej pokazuje się bez kradzieży fokusu.

use broker_ui_contract::{ApprovalCard, DecisionOption, JUDGE_HELP, mode_name_pl};
use platform_contract::{SurfaceButton, SurfaceTone, SurfaceView};
use risk_classifier_contract::RiskLevel;
use safety_broker_contract::ApprovalDecision;

/// Przycisk „Odmów” (fokus startowy; `Esc` działa tak samo).
pub const BTN_DENY: u16 = 100;
/// Przycisk „Zezwól tylko teraz” (przy zmianie trybu: „Włącz: <tryb>”).
pub const BTN_ONCE: u16 = 101;
/// Przycisk „Zawsze w tym zakresie”.
pub const BTN_SCOPED: u16 = 102;

fn button_of(option: &DecisionOption) -> u16 {
    match option {
        DecisionOption::Deny => BTN_DENY,
        DecisionOption::AllowOnce => BTN_ONCE,
        DecisionOption::AllowInScope { .. } => BTN_SCOPED,
    }
}

/// Decyzja odpowiadająca przyciskowi (tylko opcje obecne na karcie).
pub fn decision_for(card: &ApprovalCard, button: u16) -> Option<ApprovalDecision> {
    card.options
        .iter()
        .find(|o| button_of(o) == button)
        .map(|o| match o {
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

fn icon(risk: RiskLevel) -> &'static str {
    match risk {
        RiskLevel::Low => "●",
        RiskLevel::Medium => "▲",
        RiskLevel::High => "⚠",
        RiskLevel::Critical => "⛔",
    }
}

/// Widok karty z wierszem stanu; zakres „zawsze” w godzinach od chwili `now_ms`.
pub fn view_of(card: &ApprovalCard, status: &str, now_ms: u64) -> SurfaceView {
    let mut badge = match card.mode_change {
        Some(_) => format!(
            "{} Tryb pracy · ryzyko {}",
            icon(card.risk),
            card.risk.label_pl()
        ),
        None => format!("{} Ryzyko: {}", icon(card.risk), card.risk.label_pl()),
    };
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
    let buttons = card
        .options
        .iter()
        .map(|o| SurfaceButton {
            id: button_of(o),
            label: match o {
                DecisionOption::Deny => "Odmów (Esc)".to_owned(),
                DecisionOption::AllowOnce => match card.mode_change {
                    Some(mode) => format!("Włącz: {}{hello}", mode_name_pl(mode)),
                    None => format!("Zezwól tylko teraz{hello}"),
                },
                DecisionOption::AllowInScope { until_ms, .. } => {
                    let hours = until_ms.saturating_sub(now_ms).div_ceil(3_600_000);
                    format!("Zawsze w tym zakresie ({hours} h){hello}")
                }
            },
        })
        .collect();
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
    }
}
