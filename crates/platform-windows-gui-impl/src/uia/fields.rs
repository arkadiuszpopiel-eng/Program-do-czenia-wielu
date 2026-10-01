//! Odczyt treści przez UIA: `TextPattern` tylko do odczytu (odmowa dla pola hasła) i prostokąty
//! widocznych pól haseł (maskowanie zrzutów).

#![allow(unsafe_code)]

use std::time::Instant;

use platform_contract::{
    ElementRef, GuiError, ScreenRect, TargetGuard, UiaPattern, UiaText, WindowId,
};
use windows::Win32::System::Variant::VARIANT;
use windows::Win32::UI::Accessibility::{
    IUIAutomationTextPattern, TreeScope_Descendants, UIA_IsOffscreenPropertyId,
    UIA_IsPasswordPropertyId, UIA_TextPatternId,
};

use super::UiaCtx;
use super::read::{flag, resolve, window_root};
use crate::win::{rect_of, win_error};

/// Tekst z `TextPattern` (tylko odczyt; odmowa dla pola hasła).
pub(crate) fn read_text(
    ctx: &mut UiaCtx,
    guard: &TargetGuard,
    element: &ElementRef,
    max_chars: usize,
    deadline: Instant,
) -> Result<UiaText, GuiError> {
    let (el, node) = resolve(ctx, guard, element, deadline)?;
    if node.is_password {
        return Err(GuiError::Policy("pole hasła — tekst niedostępny".into()));
    }
    if !node.supports(UiaPattern::Text) {
        return Err(GuiError::PatternUnsupported(format!(
            "„{}” nie ma TextPattern",
            node.name
        )));
    }
    let limit = i32::try_from(max_chars.saturating_add(1)).unwrap_or(i32::MAX);
    // SAFETY: wzorzec tekstu i zakres dokumentu — wyłącznie odczyt.
    let text = unsafe {
        let pattern: IUIAutomationTextPattern = el
            .GetCurrentPatternAs(UIA_TextPatternId)
            .map_err(|e| win_error("TextPattern", &e))?;
        let range = pattern
            .DocumentRange()
            .map_err(|e| win_error("DocumentRange", &e))?;
        range
            .GetText(limit)
            .map_err(|e| win_error("GetText", &e))?
            .to_string()
    };
    let truncated = text.chars().count() > max_chars;
    Ok(UiaText {
        text: text.chars().take(max_chars).collect(),
        truncated,
    })
}

/// Prostokąty widocznych pól haseł w oknie.
pub(crate) fn password_rects(
    ctx: &mut UiaCtx,
    guard: &TargetGuard,
    window: WindowId,
) -> Result<Vec<ScreenRect>, GuiError> {
    let root = window_root(ctx, guard, window, "pola haseł")?;
    // SAFETY: warunek i wyszukiwanie z pamięcią podręczną na wątku MTA.
    unsafe {
        let cond = ctx
            .automation
            .CreatePropertyCondition(UIA_IsPasswordPropertyId, &VARIANT::from(true))
            .map_err(|e| win_error("CreatePropertyCondition", &e))?;
        let all = root
            .FindAllBuildCache(TreeScope_Descendants, &cond, &ctx.request)
            .map_err(|e| win_error("FindAll", &e))?;
        let n = all.Length().unwrap_or(0);
        let mut out = Vec::new();
        for i in 0..n {
            if let Ok(el) = all.GetElement(i)
                && !flag(&el, UIA_IsOffscreenPropertyId)
                && let Ok(r) = el.CachedBoundingRectangle()
            {
                out.push(rect_of(r));
            }
        }
        Ok(out)
    }
}
