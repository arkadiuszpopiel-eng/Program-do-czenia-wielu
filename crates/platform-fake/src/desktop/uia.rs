//! UIA wirtualnego pulpitu: elementy w kolejności preorder z głębokością, wyszukiwanie, tekst,
//! akcje przez wzorce (te same sprawdzenia co Windows: `UiaAction::check`, strażnik celów,
//! redakcja pól haseł), symulacja zawieszenia (limit czasu).

use platform_contract::{
    ElementRef, ExpandState, GuiError, ScreenRect, ToggleState, TreeOptions, UIA_CALL_TIMEOUT_MS,
    UiaAction, UiaNode, UiaPattern, UiaPort, UiaQuery, UiaText, UiaTree, WindowId,
};

use super::{FakeDesktop, GuiRecordKind, State};

/// Element do dodania (budowniczy).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FakeElement {
    pub(super) node: UiaNode,
    pub(super) text: Option<String>,
    pub(super) foreign_pid: Option<u32>,
}

impl FakeElement {
    /// Element o nazwie, roli i prostokącie (włączony, widoczny, bez wzorców, głębokość 1).
    pub fn new(name: &str, role: &str, rect: ScreenRect) -> Self {
        Self {
            node: UiaNode {
                element: ElementRef {
                    window: WindowId(0),
                    runtime_id: vec![0],
                },
                depth: 1,
                pid: 0,
                role: role.into(),
                name: name.into(),
                automation_id: String::new(),
                class_name: String::new(),
                value: None,
                is_password: false,
                enabled: true,
                offscreen: false,
                focused: false,
                toggle: None,
                expand: None,
                selected: None,
                rect,
                patterns: Vec::new(),
            },
            text: None,
            foreign_pid: None,
        }
    }

    /// Wzorce.
    #[must_use]
    pub fn patterns(mut self, patterns: &[UiaPattern]) -> Self {
        self.node.patterns = patterns.to_vec();
        if self.node.supports(UiaPattern::Toggle) {
            self.node.toggle = Some(ToggleState::Off);
        }
        if self.node.supports(UiaPattern::ExpandCollapse) {
            self.node.expand = Some(ExpandState::Collapsed);
        }
        if self.node.supports(UiaPattern::SelectionItem) {
            self.node.selected = Some(false);
        }
        self
    }

    /// Wartość (`ValuePattern`).
    #[must_use]
    pub fn value(mut self, value: &str) -> Self {
        self.node.value = Some(value.into());
        self
    }

    /// Pole hasła.
    #[must_use]
    pub fn password(mut self) -> Self {
        self.node.is_password = true;
        self
    }

    /// Tekst dokumentu (`TextPattern`).
    #[must_use]
    pub fn text(mut self, text: &str) -> Self {
        self.text = Some(text.into());
        if !self.node.supports(UiaPattern::Text) {
            self.node.patterns.push(UiaPattern::Text);
        }
        self
    }

    /// Głębokość w drzewie.
    #[must_use]
    pub fn depth(mut self, depth: u16) -> Self {
        self.node.depth = depth;
        self
    }

    /// `AutomationId`.
    #[must_use]
    pub fn automation_id(mut self, id: &str) -> Self {
        self.node.automation_id = id.into();
        self
    }

    /// Element dostarczany przez inny proces niż okno (np. osadzony WebView).
    #[must_use]
    pub fn foreign_pid(mut self, pid: u32) -> Self {
        self.foreign_pid = Some(pid);
        self
    }

    pub(super) fn attach(mut self, element: ElementRef, window_pid: u32) -> Self {
        self.node.element = element;
        self.node.pid = self.foreign_pid.unwrap_or(window_pid);
        self
    }
}

fn timeout(op: &str) -> GuiError {
    GuiError::Timeout {
        op: format!("UIA: {op}"),
        ms: UIA_CALL_TIMEOUT_MS,
    }
}

impl FakeDesktop {
    /// Wspólne sprawdzenia: zawieszenie, istnienie okna, strażnik (okno i proces elementu).
    fn uia_window<'a>(
        &self,
        s: &'a State,
        window: WindowId,
        op: &str,
    ) -> Result<&'a super::Win, GuiError> {
        if s.uia_hang {
            return Err(timeout(op));
        }
        let w = s.win(window)?;
        self.guard.check(w.info.pid, &w.info.image, op)?;
        Ok(w)
    }

    fn uia_element(
        &self,
        s: &State,
        element: &ElementRef,
        op: &str,
    ) -> Result<FakeElement, GuiError> {
        let w = self.uia_window(s, element.window, op)?;
        let e = w
            .elements
            .iter()
            .find(|e| e.node.element == *element)
            .ok_or_else(|| GuiError::ElementNotFound(element.to_string()))?;
        let image = if e.node.pid == w.info.pid {
            w.info.image.as_str()
        } else {
            ""
        };
        self.guard.check(e.node.pid, image, op)?;
        Ok(e.clone())
    }
}

impl UiaPort for FakeDesktop {
    fn tree(&self, window: WindowId, options: &TreeOptions) -> Result<UiaTree, GuiError> {
        let s = self.lock();
        let w = self.uia_window(&s, window, "drzewo UIA")?;
        let visible: Vec<&FakeElement> = w
            .elements
            .iter()
            .filter(|e| e.node.depth <= options.max_depth)
            .filter(|e| options.include_offscreen || !e.node.offscreen)
            .collect();
        let truncated = visible.len() > options.max_nodes
            || w.elements.iter().any(|e| e.node.depth > options.max_depth);
        Ok(UiaTree {
            window,
            nodes: visible
                .into_iter()
                .take(options.max_nodes)
                .map(|e| e.node.clone().redacted())
                .collect(),
            truncated,
        })
    }

    fn find(&self, window: WindowId, query: &UiaQuery) -> Result<Vec<UiaNode>, GuiError> {
        if query.is_empty() {
            return Err(GuiError::Policy(
                "podaj co najmniej jedno kryterium wyszukiwania".into(),
            ));
        }
        let s = self.lock();
        let w = self.uia_window(&s, window, "wyszukiwanie UIA")?;
        Ok(w.elements
            .iter()
            .filter(|e| query.matches(&e.node))
            .take(query.max_results.max(1))
            .map(|e| e.node.clone().redacted())
            .collect())
    }

    fn element(&self, element: &ElementRef) -> Result<UiaNode, GuiError> {
        let s = self.lock();
        Ok(self
            .uia_element(&s, element, "element UIA")?
            .node
            .redacted())
    }

    fn read_text(&self, element: &ElementRef, max_chars: usize) -> Result<UiaText, GuiError> {
        let s = self.lock();
        let e = self.uia_element(&s, element, "tekst UIA")?;
        if e.node.is_password {
            return Err(GuiError::Policy("pole hasła — tekst niedostępny".into()));
        }
        let text = e.text.ok_or_else(|| {
            GuiError::PatternUnsupported(format!("„{}” nie ma TextPattern", e.node.name))
        })?;
        let truncated = text.chars().count() > max_chars;
        Ok(UiaText {
            text: text.chars().take(max_chars).collect(),
            truncated,
        })
    }

    fn act(&self, element: &ElementRef, action: &UiaAction) -> Result<UiaNode, GuiError> {
        let mut s = self.lock();
        let e = self.uia_element(&s, element, "akcja UIA")?;
        action.check(&e.node)?;
        let w = s.win_mut(element.window)?;
        let Some(target) = w.elements.iter_mut().find(|x| x.node.element == *element) else {
            return Err(GuiError::ElementNotFound(element.to_string()));
        };
        let n = &mut target.node;
        match action {
            UiaAction::SetValue { value } => n.value = Some(value.clone()),
            UiaAction::Toggle => {
                n.toggle = Some(match n.toggle {
                    Some(ToggleState::On) => ToggleState::Off,
                    _ => ToggleState::On,
                });
            }
            UiaAction::Expand => n.expand = Some(ExpandState::Expanded),
            UiaAction::Collapse => n.expand = Some(ExpandState::Collapsed),
            UiaAction::Select => n.selected = Some(true),
            UiaAction::Invoke | UiaAction::Scroll { .. } => {}
        }
        let after = n.clone().redacted();
        s.record(element.window, GuiRecordKind::Uia(action.name().into()));
        Ok(after)
    }

    fn password_rects(&self, window: WindowId) -> Result<Vec<ScreenRect>, GuiError> {
        let s = self.lock();
        if s.uia_hang || s.password_check_fails.contains(&window) {
            return Err(timeout("pola haseł"));
        }
        let w = s.win(window)?;
        Ok(w.elements
            .iter()
            .filter(|e| e.node.is_password && !e.node.offscreen)
            .map(|e| e.node.rect)
            .collect())
    }
}
