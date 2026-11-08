//! Źródło tekstu z okna na pierwszym planie przez UIA (tylko odczyt):
//! - dokument: element z fokusem z `TextPattern` (albo pierwszy dokument/pole edycji z nim),
//!   `UiaPort::read_text` (port sam odmawia dla pól haseł);
//! - zaznaczenie: [`SelectionReader`] (UIA `TextPattern.GetSelection`), a gdy go brak / nic nie
//!   zwraca — zapas Ctrl+C: kopia zawartości schowka, `SendInput` Ctrl+C do okna celu (strażnik
//!   celów portu), odczyt, **przywrócenie poprzedniej zawartości**. Zapas tylko, gdy pole hasła
//!   jest wykluczone (fail-closed, jak dyktowanie).
//!
//! Okna Alfy/Brokera i procesów nieznanych — odmowa (`TargetGuard`).

use std::sync::Arc;

use platform_contract::{
    ChordKey, ClipboardContent, ClipboardPort, DesktopPort, InputControl, InputPlan, InputPort,
    InputStep, KeyChord, TreeOptions, UiaNode, UiaPattern, UiaPort, WindowId, image_file_name,
};
use voice_readaloud_contract::{
    ReadAloudError, ReadScope, RefuseReason, SelectionReader, SourceText, TextSource, UntrustedText,
};

const TREE: TreeOptions = TreeOptions {
    max_depth: 32,
    max_nodes: 2_000,
    include_offscreen: true,
};

/// Zapas Ctrl+C (wejście + schowek).
#[derive(Clone)]
pub struct CopyFallback {
    /// Wejście syntetyczne.
    pub input: Arc<dyn InputPort>,
    /// Schowek.
    pub clipboard: Arc<dyn ClipboardPort>,
}

/// Źródło tekstu na portach platformy.
#[derive(Clone)]
pub struct UiaTextSource {
    desktop: Arc<dyn DesktopPort>,
    uia: Arc<dyn UiaPort>,
    selection: Option<Arc<dyn SelectionReader>>,
    copy: Option<CopyFallback>,
}

fn refused(r: RefuseReason) -> ReadAloudError {
    ReadAloudError::Refused(r)
}

fn platform(e: impl std::fmt::Display) -> ReadAloudError {
    ReadAloudError::Platform(e.to_string())
}

impl UiaTextSource {
    /// Źródło (bez zaznaczenia UIA i bez zapasu — tylko dokument).
    pub fn new(desktop: Arc<dyn DesktopPort>, uia: Arc<dyn UiaPort>) -> Self {
        Self {
            desktop,
            uia,
            selection: None,
            copy: None,
        }
    }

    /// Port zaznaczenia UIA.
    #[must_use]
    pub fn with_selection(mut self, reader: Arc<dyn SelectionReader>) -> Self {
        self.selection = Some(reader);
        self
    }

    /// Zapas Ctrl+C.
    #[must_use]
    pub fn with_copy_fallback(mut self, copy: CopyFallback) -> Self {
        self.copy = Some(copy);
        self
    }

    fn focused(&self, window: WindowId) -> Result<(Vec<UiaNode>, Option<UiaNode>), ReadAloudError> {
        let tree = self.uia.tree(window, &TREE).map_err(platform)?;
        let focus = tree.nodes.iter().rfind(|n| n.focused).cloned();
        Ok((tree.nodes, focus))
    }

    /// Fail-closed: fokus w polu hasła albo (bez ustalonego fokusu) jakiekolwiek pole hasła.
    fn password_risk(&self, window: WindowId, focus: Option<&UiaNode>) -> bool {
        match focus {
            Some(n) => n.is_password,
            None => match self.uia.password_rects(window) {
                Ok(r) => !r.is_empty(),
                Err(_) => true,
            },
        }
    }

    fn document(&self, window: WindowId, max: usize) -> Result<(String, bool), ReadAloudError> {
        let (nodes, focus) = self.focused(window)?;
        if focus.as_ref().is_some_and(|n| n.is_password) {
            return Err(refused(RefuseReason::PasswordField));
        }
        let pick = focus
            .clone()
            .filter(|n| n.supports(UiaPattern::Text))
            .or_else(|| {
                nodes
                    .into_iter()
                    .filter(|n| n.supports(UiaPattern::Text) && !n.is_password)
                    .min_by_key(|n| u8::from(!matches!(n.role.as_str(), "document" | "edit")))
            });
        let Some(pick) = pick else {
            // Nic do czytania poza polem hasła — mówimy to wprost (bez treści).
            return Err(if self.password_risk(window, focus.as_ref()) {
                refused(RefuseReason::PasswordField)
            } else {
                refused(RefuseReason::Unsupported)
            });
        };
        let t = self
            .uia
            .read_text(&pick.element, max)
            .map_err(|e| match e {
                platform_contract::GuiError::Policy(_) => refused(RefuseReason::PasswordField),
                other => platform(other),
            })?;
        Ok((t.text, t.truncated))
    }

    fn copy_selection(
        &self,
        window: WindowId,
        copy: &CopyFallback,
    ) -> Result<String, ReadAloudError> {
        let before = copy.clipboard.get().map_err(platform)?;
        // Schowek czyszczony przed kopią — inaczej „brak zaznaczenia” przeczytałby starą zawartość.
        copy.clipboard
            .set(ClipboardContent::Empty)
            .map_err(platform)?;
        let ctrl_c = KeyChord {
            ctrl: true,
            alt: false,
            shift: false,
            win: false,
            key: ChordKey::Letter('C'),
        };
        let plan = InputPlan {
            window,
            steps: vec![InputStep::Keys { chord: ctrl_c }],
        };
        let sent = copy.input.send(&plan, &InputControl::new());
        let after = copy.clipboard.get();
        // Przywrócenie zawartości sprzed kopii (zawsze, także po błędzie).
        copy.clipboard.set(before).map_err(platform)?;
        sent.map_err(platform)?;
        match after.map_err(platform)? {
            ClipboardContent::Text(t) if !t.trim().is_empty() => Ok(t),
            _ => Err(refused(RefuseReason::NoText)),
        }
    }

    fn selection(&self, window: WindowId, max: usize) -> Result<(String, bool), ReadAloudError> {
        let (_, focus) = self.focused(window)?;
        if focus.as_ref().is_some_and(|n| n.is_password) {
            return Err(refused(RefuseReason::PasswordField));
        }
        if let Some(reader) = &self.selection
            && let Some(t) = reader.selection(window, max).map_err(platform)?
            && !t.trim().is_empty()
        {
            let truncated = t.chars().count() > max;
            return Ok((t.chars().take(max).collect(), truncated));
        }
        let copy = self
            .copy
            .as_ref()
            .ok_or_else(|| refused(RefuseReason::Unsupported))?;
        if self.password_risk(window, focus.as_ref()) {
            return Err(refused(RefuseReason::PasswordField));
        }
        let t = self.copy_selection(window, copy)?;
        let truncated = t.chars().count() > max;
        Ok((t.chars().take(max).collect(), truncated))
    }
}

impl TextSource for UiaTextSource {
    fn read(&self, scope: ReadScope, max_chars: usize) -> Result<SourceText, ReadAloudError> {
        let w = self
            .desktop
            .foreground()
            .map_err(platform)?
            .ok_or_else(|| refused(RefuseReason::NoForeground))?;
        if w.protected || self.desktop.guard().is_protected(w.pid, &w.image) {
            return Err(refused(RefuseReason::ProtectedTarget));
        }
        let (text, truncated) = match scope {
            ReadScope::Document => self.document(w.id, max_chars)?,
            ReadScope::Selection => self.selection(w.id, max_chars)?,
        };
        if text.trim().is_empty() {
            return Err(refused(RefuseReason::NoText));
        }
        Ok(SourceText {
            text: UntrustedText::new(text),
            app: image_file_name(&w.image),
            scope,
            truncated,
        })
    }
}
