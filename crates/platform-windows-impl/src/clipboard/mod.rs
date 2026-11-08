//! `ClipboardPort`: tekst (`CF_UNICODETEXT`), pliki (`CF_HDROP`), obraz (`PNG`, odczyt także
//! `CF_DIB`/`CF_DIBV5` → PNG). Treść oznaczona przez menedżery haseł
//! (`ExcludeClipboardContentFromMonitorProcessing`) nie jest odczytywana; zapisy Alfy domyślnie
//! nie trafiają do historii schowka ani chmury (`CanIncludeInClipboardHistory = 0`).

mod image;
#[cfg(windows)]
mod win;

use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard};

use platform_contract::{ClipboardContent, ClipboardPort, PlatformError};
use serde::{Deserialize, Serialize};

use crate::fs::DenyPolicy;

/// Konfiguracja schowka.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClipboardConfig {
    /// Zapisy Alfy z `CanIncludeInClipboardHistory = 0` i `CanUploadToCloudClipboard = 0`.
    pub exclude_from_history: bool,
}

impl Default for ClipboardConfig {
    fn default() -> Self {
        Self {
            exclude_from_history: true,
        }
    }
}

/// Znaczniki prywatności dopisywane do zapisu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Privacy {
    /// Bez historii schowka i synchronizacji w chmurze.
    pub(crate) exclude_history: bool,
    /// Treść poufna: także `ExcludeClipboardContentFromMonitorProcessing`.
    pub(crate) sensitive: bool,
}

/// Schowek Windows z jedną pozycją „poprzednia zawartość” do `restore_previous`.
#[derive(Debug)]
pub struct WinClipboard {
    config: ClipboardConfig,
    policy: DenyPolicy,
    previous: Mutex<Option<ClipboardContent>>,
}

impl WinClipboard {
    pub(crate) fn new(config: ClipboardConfig, policy: DenyPolicy) -> Self {
        Self {
            config,
            policy,
            previous: Mutex::new(None),
        }
    }

    fn lock(&self) -> MutexGuard<'_, Option<ClipboardContent>> {
        self.previous.lock().unwrap_or_else(|p| p.into_inner())
    }

    /// Zapis treści poufnej (np. hasła podanego przez użytkownika): poza historią, chmurą
    /// i monitorami schowka.
    pub fn set_sensitive(&self, content: ClipboardContent) -> Result<(), PlatformError> {
        self.write_remembering(
            content,
            Privacy {
                exclude_history: true,
                sensitive: true,
            },
        )
    }

    fn check_files(&self, content: &ClipboardContent) -> Result<(), PlatformError> {
        if let ClipboardContent::Files(files) = content {
            if let Some(denied) = files.iter().find(|f| self.policy.is_denied(f)) {
                return Err(PlatformError::Denylisted(denied.clone()));
            }
            if let Some(relative) = files.iter().find(|f| !f.is_absolute()) {
                return Err(PlatformError::InvalidPath(relative.clone()));
            }
        }
        Ok(())
    }

    fn write_remembering(
        &self,
        content: ClipboardContent,
        privacy: Privacy,
    ) -> Result<(), PlatformError> {
        self.check_files(&content)?;
        // Treść nieczytelna (poufna, nieobsługiwany format) nie jest zapamiętywana.
        let previous = self.get().ok();
        write(&content, privacy)?;
        *self.lock() = previous;
        Ok(())
    }
}

#[cfg(windows)]
fn write(content: &ClipboardContent, privacy: Privacy) -> Result<(), PlatformError> {
    win::write(content, privacy)
}

#[cfg(not(windows))]
fn write(_content: &ClipboardContent, _privacy: Privacy) -> Result<(), PlatformError> {
    Err(PlatformError::Unsupported(
        "schowek tylko na Windows".into(),
    ))
}

impl ClipboardPort for WinClipboard {
    fn get(&self) -> Result<ClipboardContent, PlatformError> {
        #[cfg(windows)]
        {
            win::read()
        }
        #[cfg(not(windows))]
        {
            Err(PlatformError::Unsupported(
                "schowek tylko na Windows".into(),
            ))
        }
    }

    fn set(&self, content: ClipboardContent) -> Result<(), PlatformError> {
        self.write_remembering(
            content,
            Privacy {
                exclude_history: self.config.exclude_from_history,
                sensitive: false,
            },
        )
    }

    fn restore_previous(&self) -> Result<bool, PlatformError> {
        let Some(previous) = self.lock().take() else {
            return Ok(false);
        };
        let privacy = Privacy {
            exclude_history: self.config.exclude_from_history,
            sensitive: false,
        };
        write(&previous, privacy).inspect_err(|_| *self.lock() = Some(previous.clone()))?;
        Ok(true)
    }
}

/// Struktura `DROPFILES` (20 B, `fWide = 1`) + ścieżki UTF-16 zakończone podwójnym zerem.
pub(crate) fn dropfiles_bytes(paths: &[PathBuf]) -> Vec<u8> {
    let mut out = Vec::new();
    for field in [20i32, 0, 0, 0, 1] {
        out.extend_from_slice(&field.to_le_bytes());
    }
    for path in paths {
        for unit in path.to_string_lossy().encode_utf16().chain(Some(0)) {
            out.extend_from_slice(&unit.to_le_bytes());
        }
    }
    out.extend_from_slice(&0u16.to_le_bytes());
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dropfiles_layout() {
        let bytes = dropfiles_bytes(&[PathBuf::from("C:\\a"), PathBuf::from("D:\\ż")]);
        assert_eq!(&bytes[0..4], &20i32.to_le_bytes());
        assert_eq!(&bytes[16..20], &1i32.to_le_bytes());
        let units: Vec<u16> = bytes[20..]
            .chunks_exact(2)
            .map(|c| u16::from_le_bytes([c[0], c[1]]))
            .collect();
        let text = String::from_utf16(&units).unwrap();
        assert_eq!(text, "C:\\a\0D:\\ż\0\0");
    }

    #[test]
    fn denied_and_relative_files_are_rejected_before_touching_clipboard() {
        let clip = WinClipboard::new(ClipboardConfig::default(), DenyPolicy::default());
        let denied = ClipboardContent::Files(vec![PathBuf::from("/home/u/.ssh/id_rsa")]);
        assert!(matches!(
            clip.set(denied),
            Err(PlatformError::Denylisted(_))
        ));
        let relative = ClipboardContent::Files(vec![PathBuf::from("rel.txt")]);
        assert!(matches!(
            clip.set(relative),
            Err(PlatformError::InvalidPath(_))
        ));
        assert!(!clip.restore_previous().unwrap());
        if !cfg!(windows) {
            assert!(matches!(clip.get(), Err(PlatformError::Unsupported(_))));
            let text = ClipboardContent::Text("x".into());
            assert!(clip.set_sensitive(text).is_err());
        }
    }
}
