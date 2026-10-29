//! Powłoki zainstalowane na tym komputerze: PowerShell 7 (`pwsh`) nie jest częścią Windows —
//! bez niego każde `shell_run` z domyślną powłoką kończyło się „nie znaleziono ścieżki”
//! (pomiar F3 na laptopie bez PowerShell 7, 2026-10-29).

use std::path::Path;

use crate::{ShellKind, ShellToolsConfig};

impl ShellToolsConfig {
    /// Konfiguracja domyślna dopasowana do zainstalowanych powłok.
    pub fn detected() -> Self {
        Self::default().with_installed(|p| Path::new(p).is_file())
    }

    /// Bez `pwsh` domyślną powłoką jest Windows PowerShell, a jawne `pwsh` uruchamia
    /// `powershell.exe` (ta sama składnia poleceń dla analizy) zamiast kończyć się błędem.
    #[must_use]
    pub fn with_installed(mut self, exists: impl Fn(&str) -> bool) -> Self {
        if !exists(&self.pwsh_path) && exists(&self.powershell_path) {
            self.pwsh_path.clone_from(&self.powershell_path);
            if self.default_shell == ShellKind::Pwsh {
                self.default_shell = ShellKind::Powershell;
            }
        }
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_pwsh_falls_back_to_windows_powershell() {
        let base = ShellToolsConfig::default();
        let c = base.clone().with_installed(|p| !p.ends_with("pwsh.exe"));
        assert_eq!(c.default_shell, ShellKind::Powershell);
        assert_eq!(c.pwsh_path, base.powershell_path);
        let (program, _, _) = c.invocation(ShellKind::Pwsh, "Get-Date");
        assert!(program.ends_with("powershell.exe"), "{program}");
        // Z PowerShell 7 — bez zmian; bez żadnego PowerShella też bez zmian (błąd mówi prawdę).
        assert_eq!(base.clone().with_installed(|_| true), base);
        assert_eq!(base.clone().with_installed(|_| false), base);
    }
}
