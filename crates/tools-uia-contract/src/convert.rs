//! Konwersje między argumentami/wynikami narzędzi a typami portu UIA (`platform-contract`)
//! i zwięzły tekst drzewa dla modelu (wcięcia = głębokość; nazwy i wartości zredagowane).

use platform_contract::{
    ElementRef, ExpandState, ScrollAmount, ScrollDirection, ToggleState, UiaAction, UiaNode,
    UiaPattern, UiaQuery,
};
use tools_common_contract::text::redact_secrets;

use crate::{ActArgs, ActionArg, AmountArg, DirectionArg, FindArgs, NodeOut};

fn pattern_name(p: UiaPattern) -> &'static str {
    match p {
        UiaPattern::Invoke => "invoke",
        UiaPattern::Value => "set_value",
        UiaPattern::Toggle => "toggle",
        UiaPattern::ExpandCollapse => "expand_collapse",
        UiaPattern::SelectionItem => "select",
        UiaPattern::Scroll => "scroll",
        UiaPattern::Text => "read_text",
    }
}

/// Węzeł portu → wynik narzędzia (sekrety w nazwie i wartości zredagowane; hasło bez wartości).
pub fn node_out(n: &UiaNode) -> NodeOut {
    NodeOut {
        element: n.element.to_string(),
        depth: n.depth,
        role: n.role.clone(),
        name: redact_secrets(&n.name),
        automation_id: n.automation_id.clone(),
        value: if n.is_password {
            None
        } else {
            n.value.as_deref().map(redact_secrets)
        },
        password: n.is_password,
        enabled: n.enabled,
        focused: n.focused,
        toggle: n.toggle.map(|t| match t {
            ToggleState::On => "on".into(),
            ToggleState::Off => "off".into(),
            ToggleState::Indeterminate => "indeterminate".into(),
        }),
        expand: n.expand.map(|e| match e {
            ExpandState::Collapsed => "collapsed".into(),
            ExpandState::Expanded => "expanded".into(),
            ExpandState::PartiallyExpanded => "partially_expanded".into(),
            ExpandState::LeafNode => "leaf".into(),
        }),
        selected: n.selected,
        x: n.rect.left,
        y: n.rect.top,
        width: n.rect.width(),
        height: n.rect.height(),
        patterns: n
            .patterns
            .iter()
            .map(|p| pattern_name(*p).to_owned())
            .collect(),
    }
}

/// Tekst dla modelu: jedna linia na element.
pub fn render_nodes(nodes: &[NodeOut]) -> String {
    let mut out = String::new();
    for n in nodes {
        let indent = "  ".repeat(usize::from(n.depth.min(30)));
        let mut line = format!("{indent}- {} „{}” [{}]", n.role, n.name, n.element);
        if let Some(v) = &n.value {
            line.push_str(&format!(" wartość=„{v}”"));
        }
        if n.password {
            line.push_str(" (pole hasła)");
        }
        if !n.enabled {
            line.push_str(" (wyłączony)");
        }
        if !n.patterns.is_empty() {
            line.push_str(&format!(" akcje: {}", n.patterns.join(",")));
        }
        out.push_str(&line);
        out.push('\n');
    }
    out
}

/// Argumenty `uia_act` → akcja portu (wymagane pola: `value` dla `set_value`, `direction` dla `scroll`).
pub fn to_action(a: &ActArgs) -> Result<UiaAction, String> {
    ElementRef::parse(&a.element).map_err(|e| e.to_string())?;
    if a.value.is_some() && a.action != ActionArg::SetValue {
        return Err("`value` tylko dla `set_value`".into());
    }
    Ok(match a.action {
        ActionArg::Invoke => UiaAction::Invoke,
        ActionArg::SetValue => UiaAction::SetValue {
            value: a.value.clone().ok_or("`set_value` wymaga `value`")?,
        },
        ActionArg::Toggle => UiaAction::Toggle,
        ActionArg::Expand => UiaAction::Expand,
        ActionArg::Collapse => UiaAction::Collapse,
        ActionArg::Select => UiaAction::Select,
        ActionArg::Scroll => UiaAction::Scroll {
            direction: match a.direction.ok_or("`scroll` wymaga `direction`")? {
                DirectionArg::Up => ScrollDirection::Up,
                DirectionArg::Down => ScrollDirection::Down,
                DirectionArg::Left => ScrollDirection::Left,
                DirectionArg::Right => ScrollDirection::Right,
            },
            amount: match a.amount.unwrap_or(AmountArg::Small) {
                AmountArg::Small => ScrollAmount::Small,
                AmountArg::Large => ScrollAmount::Large,
            },
        },
    })
}

/// Argumenty `uia_find` → zapytanie portu (co najmniej jedno kryterium, limit wyników).
pub fn to_query(a: &FindArgs, max: u32) -> Result<UiaQuery, String> {
    let q = UiaQuery {
        name: a.name.clone(),
        name_contains: a.name_contains.clone(),
        role: a.role.clone(),
        automation_id: a.automation_id.clone(),
        class_name: a.class_name.clone(),
        max_results: usize::try_from(a.max_results.unwrap_or(10).clamp(1, max.max(1)))
            .unwrap_or(10),
    };
    if q.is_empty() {
        return Err("podaj co najmniej jedno kryterium (`name`, `name_contains`, `role`, `automation_id`, `class_name`)".into());
    }
    Ok(q)
}
