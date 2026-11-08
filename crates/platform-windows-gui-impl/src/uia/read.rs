//! Odczyt UIA na wątku roboczym: żądanie pamięci podręcznej (jedno wywołanie międzyprocesowe na
//! węzeł zamiast kilkunastu), przejście widoku kontrolek z limitem węzłów, głębokości i czasu,
//! wyszukiwanie, rozwiązywanie odwołań (pamięć podręczna → przeszukanie poddrzewa okna), pola
//! haseł, `TextPattern` tylko do odczytu. Strażnik: właściciel okna i proces każdego elementu.

#![allow(unsafe_code)]

use std::collections::BTreeMap;
use std::time::Instant;

use platform_contract::{
    ElementRef, ExpandState, GuiError, TargetGuard, ToggleState, TreeOptions, UiaNode, UiaPattern,
    UiaQuery, UiaTree, WindowId, control_type_name,
};
use windows::Win32::System::Com::CoTaskMemFree;
use windows::Win32::System::Variant::{VARIANT, VariantToInt32ArrayAlloc};
use windows::Win32::UI::Accessibility::{
    IUIAutomation, IUIAutomationCacheRequest, IUIAutomationElement, UIA_AutomationIdPropertyId,
    UIA_BoundingRectanglePropertyId, UIA_ClassNamePropertyId, UIA_ControlTypePropertyId,
    UIA_ExpandCollapseExpandCollapseStatePropertyId, UIA_HasKeyboardFocusPropertyId,
    UIA_IsEnabledPropertyId, UIA_IsExpandCollapsePatternAvailablePropertyId,
    UIA_IsInvokePatternAvailablePropertyId, UIA_IsOffscreenPropertyId, UIA_IsPasswordPropertyId,
    UIA_IsScrollPatternAvailablePropertyId, UIA_IsSelectionItemPatternAvailablePropertyId,
    UIA_IsTextPatternAvailablePropertyId, UIA_IsTogglePatternAvailablePropertyId,
    UIA_IsValuePatternAvailablePropertyId, UIA_NamePropertyId, UIA_PROPERTY_ID,
    UIA_ProcessIdPropertyId, UIA_RuntimeIdPropertyId, UIA_SelectionItemIsSelectedPropertyId,
    UIA_ToggleToggleStatePropertyId, UIA_ValueValuePropertyId,
};
use windows::core::BSTR;

use super::UiaCtx;
use crate::links::{ProcessTree, check_pid, pid_protected, window_target};
use crate::win::{hwnd_of, rect_of, win_error, window_pid};

/// Limit węzłów przy wyszukiwaniu i rozwiązywaniu odwołań.
const SEARCH_LIMIT: usize = 5_000;

const PATTERN_PROPS: [(UIA_PROPERTY_ID, UiaPattern); 7] = [
    (UIA_IsInvokePatternAvailablePropertyId, UiaPattern::Invoke),
    (UIA_IsValuePatternAvailablePropertyId, UiaPattern::Value),
    (UIA_IsTogglePatternAvailablePropertyId, UiaPattern::Toggle),
    (
        UIA_IsExpandCollapsePatternAvailablePropertyId,
        UiaPattern::ExpandCollapse,
    ),
    (
        UIA_IsSelectionItemPatternAvailablePropertyId,
        UiaPattern::SelectionItem,
    ),
    (UIA_IsScrollPatternAvailablePropertyId, UiaPattern::Scroll),
    (UIA_IsTextPatternAvailablePropertyId, UiaPattern::Text),
];

/// Żądanie pamięci podręcznej z właściwościami potrzebnymi do `UiaNode`.
pub(crate) fn cache_request(
    automation: &IUIAutomation,
) -> Result<IUIAutomationCacheRequest, GuiError> {
    // SAFETY: wywołania COM na wątku MTA, który utworzył `automation`.
    unsafe {
        let r = automation
            .CreateCacheRequest()
            .map_err(|e| win_error("CreateCacheRequest", &e))?;
        for p in [
            UIA_RuntimeIdPropertyId,
            UIA_ProcessIdPropertyId,
            UIA_ControlTypePropertyId,
            UIA_NamePropertyId,
            UIA_AutomationIdPropertyId,
            UIA_ClassNamePropertyId,
            UIA_IsEnabledPropertyId,
            UIA_IsOffscreenPropertyId,
            UIA_HasKeyboardFocusPropertyId,
            UIA_IsPasswordPropertyId,
            UIA_BoundingRectanglePropertyId,
            UIA_ValueValuePropertyId,
            UIA_ToggleToggleStatePropertyId,
            UIA_ExpandCollapseExpandCollapseStatePropertyId,
            UIA_SelectionItemIsSelectedPropertyId,
        ]
        .into_iter()
        .chain(PATTERN_PROPS.iter().map(|(p, _)| *p))
        {
            r.AddProperty(p).map_err(|e| win_error("AddProperty", &e))?;
        }
        Ok(r)
    }
}

fn cached(el: &IUIAutomationElement, id: UIA_PROPERTY_ID) -> Option<VARIANT> {
    // SAFETY: odczyt właściwości z pamięci podręcznej elementu (bez wywołania międzyprocesowego).
    unsafe { el.GetCachedPropertyValue(id) }
        .ok()
        .filter(|v| !v.is_empty())
}

pub(super) fn flag(el: &IUIAutomationElement, id: UIA_PROPERTY_ID) -> bool {
    cached(el, id)
        .and_then(|v| bool::try_from(&v).ok())
        .unwrap_or(false)
}

fn int(el: &IUIAutomationElement, id: UIA_PROPERTY_ID) -> Option<i32> {
    cached(el, id).and_then(|v| i32::try_from(&v).ok())
}

fn text(el: &IUIAutomationElement, id: UIA_PROPERTY_ID) -> String {
    cached(el, id)
        .and_then(|v| BSTR::try_from(&v).ok())
        .map(|b| b.to_string())
        .unwrap_or_default()
}

/// `RuntimeId` z pamięci podręcznej.
pub(crate) fn runtime_id(el: &IUIAutomationElement) -> Option<Vec<i32>> {
    let v = cached(el, UIA_RuntimeIdPropertyId)?;
    let mut ptr: *mut i32 = std::ptr::null_mut();
    let mut n = 0u32;
    // SAFETY: konwersja tablicy z VARIANT do bufora CoTaskMem, który zwalniamy poniżej.
    unsafe { VariantToInt32ArrayAlloc(&raw const v, &raw mut ptr, &raw mut n) }.ok()?;
    if ptr.is_null() {
        return None;
    }
    // SAFETY: `ptr` wskazuje `n` liczb i32 zaalokowanych przez system.
    let ids = unsafe { std::slice::from_raw_parts(ptr, n as usize) }.to_vec();
    // SAFETY: zwolnienie bufora z `VariantToInt32ArrayAlloc`.
    unsafe { CoTaskMemFree(Some(ptr.cast())) };
    (!ids.is_empty()).then_some(ids)
}

/// Węzeł z elementu z wypełnioną pamięcią podręczną (wartość pola hasła usunięta).
pub(crate) fn node_of(el: &IUIAutomationElement, window: WindowId, depth: u16) -> Option<UiaNode> {
    let runtime_id = runtime_id(el)?;
    let has = |p: UiaPattern| PATTERN_PROPS.iter().any(|(id, q)| *q == p && flag(el, *id));
    let patterns: Vec<UiaPattern> = PATTERN_PROPS
        .iter()
        .filter(|(id, _)| flag(el, *id))
        .map(|(_, p)| *p)
        .collect();
    // SAFETY: prostokąt z pamięci podręcznej.
    let rect = unsafe { el.CachedBoundingRectangle() }
        .map(rect_of)
        .unwrap_or_default();
    let is_password = flag(el, UIA_IsPasswordPropertyId);
    let node = UiaNode {
        element: ElementRef { window, runtime_id },
        depth,
        pid: int(el, UIA_ProcessIdPropertyId)
            .and_then(|p| u32::try_from(p).ok())
            .unwrap_or(0),
        role: control_type_name(int(el, UIA_ControlTypePropertyId).unwrap_or(0)).into(),
        name: text(el, UIA_NamePropertyId),
        automation_id: text(el, UIA_AutomationIdPropertyId),
        class_name: text(el, UIA_ClassNamePropertyId),
        value: (has(UiaPattern::Value) && !is_password).then(|| text(el, UIA_ValueValuePropertyId)),
        is_password,
        enabled: flag(el, UIA_IsEnabledPropertyId),
        offscreen: flag(el, UIA_IsOffscreenPropertyId),
        focused: flag(el, UIA_HasKeyboardFocusPropertyId),
        toggle: has(UiaPattern::Toggle).then(|| match int(el, UIA_ToggleToggleStatePropertyId) {
            Some(1) => ToggleState::On,
            Some(2) => ToggleState::Indeterminate,
            _ => ToggleState::Off,
        }),
        expand: has(UiaPattern::ExpandCollapse).then(|| {
            match int(el, UIA_ExpandCollapseExpandCollapseStatePropertyId) {
                Some(1) => ExpandState::Expanded,
                Some(2) => ExpandState::PartiallyExpanded,
                Some(3) => ExpandState::LeafNode,
                _ => ExpandState::Collapsed,
            }
        }),
        selected: has(UiaPattern::SelectionItem)
            .then(|| flag(el, UIA_SelectionItemIsSelectedPropertyId)),
        rect,
        patterns,
    };
    Some(node.redacted())
}

/// Sprawdza właściciela okna strażnikiem i zwraca element główny okna (z pamięcią podręczną).
pub(super) fn window_root(
    ctx: &UiaCtx,
    guard: &TargetGuard,
    window: WindowId,
    op: &str,
) -> Result<IUIAutomationElement, GuiError> {
    let target = window_target(hwnd_of(window), &ProcessTree::snapshot());
    let hwnd = target.root;
    if window_pid(hwnd) == 0 {
        return Err(GuiError::ElementNotFound(format!("okno {}", window.0)));
    }
    target.check(guard, op)?;
    // SAFETY: element okna z pamięcią podręczną (wątek MTA tego `automation`).
    unsafe {
        ctx.automation
            .ElementFromHandleBuildCache(hwnd, &ctx.request)
    }
    .map_err(|e| win_error("ElementFromHandle", &e))
}

/// Limity przejścia.
struct Limits {
    max_depth: u16,
    nodes: usize,
    deadline: Instant,
}

/// Przejście poddrzewa (preorder) z limitami; `visit` zwraca `false`, by zakończyć. Zwraca
/// `true`, gdy przerwał limit węzłów albo czasu.
fn walk(
    ctx: &mut UiaCtx,
    guard: &TargetGuard,
    (window, root): (WindowId, IUIAutomationElement),
    limits: &Limits,
    mut visit: impl FnMut(&UiaNode) -> bool,
) -> bool {
    let window_pid = window_pid(hwnd_of(window));
    let processes = ProcessTree::snapshot();
    let mut images: BTreeMap<u32, bool> = BTreeMap::new();
    let mut stack = vec![(root, 0u16)];
    let mut seen = 0usize;
    while let Some((el, depth)) = stack.pop() {
        if seen >= limits.nodes || Instant::now() >= limits.deadline {
            return true;
        }
        seen += 1;
        let Some(node) = node_of(&el, window, depth) else {
            continue;
        };
        let protected = node.pid != window_pid
            && *images
                .entry(node.pid)
                .or_insert_with(|| pid_protected(guard, &processes, node.pid));
        if protected {
            continue;
        }
        ctx.remember(window.0, node.element.runtime_id.clone(), el.clone());
        if !visit(&node) {
            return false;
        }
        if depth >= limits.max_depth {
            continue;
        }
        let mut children = Vec::new();
        // SAFETY: nawigacja widoku kontrolek z pamięcią podręczną; brak dziecka = błąd/null.
        let mut next = unsafe { ctx.walker.GetFirstChildElementBuildCache(&el, &ctx.request) };
        while let Ok(child) = next {
            // SAFETY: jw.
            next = unsafe {
                ctx.walker
                    .GetNextSiblingElementBuildCache(&child, &ctx.request)
            };
            children.push((child, depth + 1));
        }
        stack.extend(children.into_iter().rev());
    }
    false
}

/// Drzewo okna.
pub(crate) fn tree(
    ctx: &mut UiaCtx,
    guard: &TargetGuard,
    window: WindowId,
    options: &TreeOptions,
    deadline: Instant,
) -> Result<UiaTree, GuiError> {
    let root = window_root(ctx, guard, window, "drzewo UIA")?;
    let mut nodes = Vec::new();
    let mut full = false;
    let limits = Limits {
        max_depth: options.max_depth,
        nodes: options.max_nodes.saturating_mul(4).max(1),
        deadline,
    };
    let cut = walk(ctx, guard, (window, root), &limits, |n| {
        if options.include_offscreen || !n.offscreen {
            if nodes.len() >= options.max_nodes {
                full = true;
                return false;
            }
            nodes.push(n.clone());
        }
        true
    });
    Ok(UiaTree {
        window,
        nodes,
        truncated: cut || full,
    })
}

/// Wyszukiwanie po kryteriach.
pub(crate) fn find(
    ctx: &mut UiaCtx,
    guard: &TargetGuard,
    window: WindowId,
    query: &UiaQuery,
    deadline: Instant,
) -> Result<Vec<UiaNode>, GuiError> {
    if query.is_empty() {
        return Err(GuiError::Policy(
            "podaj co najmniej jedno kryterium wyszukiwania".into(),
        ));
    }
    let root = window_root(ctx, guard, window, "wyszukiwanie UIA")?;
    let mut out = Vec::new();
    let max = query.max_results.max(1);
    let limits = Limits {
        max_depth: u16::MAX,
        nodes: SEARCH_LIMIT,
        deadline,
    };
    walk(ctx, guard, (window, root), &limits, |n| {
        if query.matches(n) {
            out.push(n.clone());
        }
        out.len() < max
    });
    Ok(out)
}

/// Rozwiązuje odwołanie: pamięć podręczna (odświeżona) albo przeszukanie poddrzewa okna;
/// sprawdza strażnikiem właściciela okna i proces elementu.
pub(crate) fn resolve(
    ctx: &mut UiaCtx,
    guard: &TargetGuard,
    element: &ElementRef,
    deadline: Instant,
) -> Result<(IUIAutomationElement, UiaNode), GuiError> {
    let root = window_root(ctx, guard, element.window, "element UIA")?;
    let fresh = |el: &IUIAutomationElement, ctx: &UiaCtx| {
        // SAFETY: odświeżenie pamięci podręcznej elementu (wywołanie międzyprocesowe).
        unsafe { el.BuildUpdatedCache(&ctx.request) }.ok()
    };
    let mut found = ctx
        .cached(element.window.0, &element.runtime_id)
        .and_then(|el| fresh(&el, ctx));
    if found.is_none() {
        ctx.forget(element.window.0, &element.runtime_id);
        let limits = Limits {
            max_depth: u16::MAX,
            nodes: SEARCH_LIMIT,
            deadline,
        };
        walk(ctx, guard, (element.window, root), &limits, |n| {
            n.element != *element
        });
        found = ctx
            .cached(element.window.0, &element.runtime_id)
            .and_then(|el| fresh(&el, ctx));
    }
    let el = found.ok_or_else(|| GuiError::ElementNotFound(element.to_string()))?;
    let node = node_of(&el, element.window, 0)
        .ok_or_else(|| GuiError::ElementNotFound(element.to_string()))?;
    if node.element.runtime_id != element.runtime_id {
        return Err(GuiError::ElementNotFound(element.to_string()));
    }
    let window_pid = window_pid(hwnd_of(element.window));
    if node.pid != window_pid {
        check_pid(guard, &ProcessTree::snapshot(), node.pid, "element UIA")?;
    }
    Ok((el, node))
}

/// Element (stan bieżący).
pub(crate) fn node_of_element(
    ctx: &mut UiaCtx,
    guard: &TargetGuard,
    element: &ElementRef,
    deadline: Instant,
) -> Result<UiaNode, GuiError> {
    resolve(ctx, guard, element, deadline).map(|(_, n)| n)
}
