//! Fake schowka.

use std::sync::Mutex;

use platform_contract::{ClipboardContent, ClipboardPort, PlatformError};

/// Schowek w pamięci z jedną pozycją historii.
#[derive(Debug)]
pub struct FakeClipboard {
    inner: Mutex<(ClipboardContent, Option<ClipboardContent>)>,
}

impl Default for FakeClipboard {
    fn default() -> Self {
        Self::new()
    }
}

impl FakeClipboard {
    /// Pusty schowek.
    pub fn new() -> Self {
        Self {
            inner: Mutex::new((ClipboardContent::Empty, None)),
        }
    }

    /// Czy jest zapamiętana poprzednia zawartość do przywrócenia.
    pub fn has_previous(&self) -> bool {
        self.lock().1.is_some()
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, (ClipboardContent, Option<ClipboardContent>)> {
        self.inner.lock().unwrap_or_else(|p| p.into_inner())
    }
}

impl ClipboardPort for FakeClipboard {
    fn get(&self) -> Result<ClipboardContent, PlatformError> {
        Ok(self.lock().0.clone())
    }

    fn set(&self, content: ClipboardContent) -> Result<(), PlatformError> {
        let mut g = self.lock();
        let previous = std::mem::replace(&mut g.0, content);
        g.1 = Some(previous);
        Ok(())
    }

    fn restore_previous(&self) -> Result<bool, PlatformError> {
        let mut g = self.lock();
        match g.1.take() {
            Some(prev) => {
                g.0 = prev;
                Ok(true)
            }
            None => Ok(false),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_and_restore() {
        let c = FakeClipboard::new();
        c.set(ClipboardContent::Text("a".into())).unwrap();
        c.set(ClipboardContent::Text("b".into())).unwrap();
        assert_eq!(c.get().unwrap(), ClipboardContent::Text("b".into()));
        assert!(c.restore_previous().unwrap());
        assert_eq!(c.get().unwrap(), ClipboardContent::Text("a".into()));
        assert!(!c.restore_previous().unwrap());
    }
}
