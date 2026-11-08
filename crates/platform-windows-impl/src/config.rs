//! Konfiguracja portu (`[platform]` w konfiguracji jądra; deny-lista i procesy chronione to
//! `kernel_policy` — poza zasięgiem agentek i Ulepszacza).

use serde::{Deserialize, Serialize};

use crate::clipboard::ClipboardConfig;
use crate::fs::FsConfig;
use crate::process::JobLimits;
use crate::window::WindowGuard;

/// Pełna konfiguracja `WindowsPlatform`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct PlatformConfig {
    /// System plików (katalog kopii do cofania, dodatkowa deny-lista).
    pub fs: FsConfig,
    /// Schowek (prywatność zapisów).
    pub clipboard: ClipboardConfig,
    /// Okna chronione przed sterowaniem.
    pub windows: WindowGuard,
    /// Limity Job Object dla każdego uruchamianego procesu (np. emulacja baseline).
    pub job_defaults: JobLimits,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_privacy_first_and_protect_alfa() {
        let c = PlatformConfig::default();
        assert!(c.clipboard.exclude_from_history);
        assert!(
            c.windows
                .protected_processes
                .iter()
                .any(|p| p == "alfa-broker.exe")
        );
        assert!(c.fs.extra_deny_names.iter().any(|n| n == ".aws"));
        assert_eq!(c.job_defaults, JobLimits::default());
    }
}
