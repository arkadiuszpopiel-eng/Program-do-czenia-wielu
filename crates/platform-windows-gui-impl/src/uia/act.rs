//! Akcje UIA wyłącznie przez wzorce (bez wejścia syntetycznego): `Invoke`, `Value.SetValue`,
//! `Toggle`, `ExpandCollapse`, `SelectionItem.Select`, `Scroll`. Przed akcją: rozwiązanie
//! odwołania ze strażnikiem (okno i proces elementu) i `UiaAction::check` (wzorzec, element
//! włączony, zakaz wpisywania w pole hasła); po akcji odświeżony element = weryfikacja (F6-04).

#![allow(unsafe_code)]

use std::time::Instant;

use platform_contract::{
    ElementRef, GuiError, ScrollAmount, ScrollDirection, TargetGuard, UiaAction, UiaNode,
};
use windows::Win32::UI::Accessibility::{
    IUIAutomationElement, IUIAutomationExpandCollapsePattern, IUIAutomationInvokePattern,
    IUIAutomationScrollPattern, IUIAutomationSelectionItemPattern, IUIAutomationTogglePattern,
    IUIAutomationValuePattern, ScrollAmount as UiaScrollAmount, ScrollAmount_LargeDecrement,
    ScrollAmount_LargeIncrement, ScrollAmount_NoAmount, ScrollAmount_SmallDecrement,
    ScrollAmount_SmallIncrement, UIA_ExpandCollapsePatternId, UIA_InvokePatternId, UIA_PATTERN_ID,
    UIA_ScrollPatternId, UIA_SelectionItemPatternId, UIA_TogglePatternId, UIA_ValuePatternId,
};
use windows::core::{BSTR, Interface};

use super::UiaCtx;
use super::read::{node_of, resolve};
use crate::win::win_error;

fn pattern<T: Interface>(
    el: &IUIAutomationElement,
    id: UIA_PATTERN_ID,
    name: &str,
) -> Result<T, GuiError> {
    // SAFETY: pobranie wzorca elementu (wątek MTA tego elementu).
    unsafe { el.GetCurrentPatternAs::<T>(id) }.map_err(|e| win_error(name, &e))
}

fn scroll_amounts(
    direction: ScrollDirection,
    amount: ScrollAmount,
) -> (UiaScrollAmount, UiaScrollAmount) {
    let (dec, inc) = match amount {
        ScrollAmount::Small => (ScrollAmount_SmallDecrement, ScrollAmount_SmallIncrement),
        ScrollAmount::Large => (ScrollAmount_LargeDecrement, ScrollAmount_LargeIncrement),
    };
    match direction {
        ScrollDirection::Up => (ScrollAmount_NoAmount, dec),
        ScrollDirection::Down => (ScrollAmount_NoAmount, inc),
        ScrollDirection::Left => (dec, ScrollAmount_NoAmount),
        ScrollDirection::Right => (inc, ScrollAmount_NoAmount),
    }
}

fn perform(el: &IUIAutomationElement, action: &UiaAction) -> Result<(), GuiError> {
    // SAFETY: wywołania metod wzorców na elemencie z tego wątku MTA.
    unsafe {
        match action {
            UiaAction::Invoke => {
                pattern::<IUIAutomationInvokePattern>(el, UIA_InvokePatternId, "Invoke")?
                    .Invoke()
                    .map_err(|e| win_error("Invoke", &e))
            }
            UiaAction::SetValue { value } => {
                pattern::<IUIAutomationValuePattern>(el, UIA_ValuePatternId, "Value")?
                    .SetValue(&BSTR::from(value.as_str()))
                    .map_err(|e| win_error("SetValue", &e))
            }
            UiaAction::Toggle => {
                pattern::<IUIAutomationTogglePattern>(el, UIA_TogglePatternId, "Toggle")?
                    .Toggle()
                    .map_err(|e| win_error("Toggle", &e))
            }
            UiaAction::Expand => pattern::<IUIAutomationExpandCollapsePattern>(
                el,
                UIA_ExpandCollapsePatternId,
                "ExpandCollapse",
            )?
            .Expand()
            .map_err(|e| win_error("Expand", &e)),
            UiaAction::Collapse => pattern::<IUIAutomationExpandCollapsePattern>(
                el,
                UIA_ExpandCollapsePatternId,
                "ExpandCollapse",
            )?
            .Collapse()
            .map_err(|e| win_error("Collapse", &e)),
            UiaAction::Select => pattern::<IUIAutomationSelectionItemPattern>(
                el,
                UIA_SelectionItemPatternId,
                "SelectionItem",
            )?
            .Select()
            .map_err(|e| win_error("Select", &e)),
            UiaAction::Scroll { direction, amount } => {
                let (h, v) = scroll_amounts(*direction, *amount);
                pattern::<IUIAutomationScrollPattern>(el, UIA_ScrollPatternId, "Scroll")?
                    .Scroll(h, v)
                    .map_err(|e| win_error("Scroll", &e))
            }
        }
    }
}

/// Akcja na elemencie; zwraca odświeżony węzeł (weryfikacja po akcji).
pub(crate) fn act(
    ctx: &mut UiaCtx,
    guard: &TargetGuard,
    element: &ElementRef,
    action: &UiaAction,
    deadline: Instant,
) -> Result<UiaNode, GuiError> {
    let (el, node) = resolve(ctx, guard, element, deadline)?;
    action.check(&node)?;
    perform(&el, action)?;
    // SAFETY: odświeżenie pamięci podręcznej po akcji.
    let after = unsafe { el.BuildUpdatedCache(&ctx.request) }
        .map_err(|e| win_error("BuildUpdatedCache", &e))?;
    node_of(&after, element.window, node.depth)
        .ok_or_else(|| GuiError::ElementNotFound(element.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scroll_mapping() {
        assert_eq!(
            scroll_amounts(ScrollDirection::Down, ScrollAmount::Small),
            (ScrollAmount_NoAmount, ScrollAmount_SmallIncrement)
        );
        assert_eq!(
            scroll_amounts(ScrollDirection::Left, ScrollAmount::Large),
            (ScrollAmount_LargeDecrement, ScrollAmount_NoAmount)
        );
    }
}
