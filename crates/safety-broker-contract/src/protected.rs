//! Ścieżki chronione (plan 50 §4.5, jak w Claude Code i Codex): repozytorium, konfiguracja
//! narzędzi i agentów, klucze, pliki startowe powłoki. Zmiana takiej ścieżki nigdy nie przechodzi
//! bez pytania poza „Pełnym dostępem”; odczyt — bez zmian (sekrety i tak pilnują deny-listy Jądra).

use compliance_contract::deny::NormPath;

/// Katalogi chronione — gdziekolwiek w ścieżce.
const DIRS: &[&str] = &[
    ".git", ".alfa", ".ssh", ".aws", ".azure", ".gnupg", ".kube", ".docker", ".codex", ".claude",
    ".agents", ".vscode", ".idea", ".husky",
];

/// Pliki chronione — po nazwie (konfiguracja git i narzędzi, pliki startowe powłoki).
const FILES: &[&str] = &[
    ".gitconfig",
    ".gitmodules",
    ".npmrc",
    ".pypirc",
    ".netrc",
    ".bashrc",
    ".bash_profile",
    ".profile",
    ".zshrc",
    ".zprofile",
    ".ripgreprc",
    ".mcp.json",
    ".claude.json",
    "profile.ps1",
    "microsoft.powershell_profile.ps1",
    "microsoft.vscode_powershell_profile.ps1",
];

/// Czy ścieżka jest chroniona (porównanie bez wielkości liter).
pub(crate) fn is_protected(p: &NormPath) -> bool {
    let Some((last, dirs)) = p.comps.split_last() else {
        return false;
    };
    let low = |s: &str| s.to_lowercase();
    dirs.iter()
        .chain(std::iter::once(last))
        .any(|c| DIRS.contains(&low(c).as_str()))
        || FILES.contains(&low(last).as_str())
}

#[cfg(test)]
mod tests {
    use super::*;
    use compliance_contract::deny::{PathEnv, normalize};

    fn prot(p: &str) -> bool {
        is_protected(&normalize(p, &PathEnv::new()))
    }

    #[test]
    fn protected_dirs_anywhere_in_path() {
        assert!(prot(r"E:\Projekt\.git"));
        assert!(prot(r"E:\Projekt\.git\config"));
        assert!(prot(r"E:\Projekt\.GIT\hooks\pre-commit"));
        assert!(prot(r"C:\Users\a\.ssh\id_ed25519"));
        assert!(prot(r"E:\Projekt\.vscode\settings.json"));
    }

    #[test]
    fn protected_files_by_name() {
        assert!(prot(r"C:\Users\a\.gitconfig"));
        assert!(prot(r"E:\Projekt\.npmrc"));
        assert!(prot(
            r"C:\Users\a\Documents\WindowsPowerShell\Microsoft.PowerShell_profile.ps1"
        ));
    }

    #[test]
    fn ordinary_paths_are_not_protected() {
        assert!(!prot(r"E:\Projekt\src\main.rs"));
        assert!(!prot(r"E:\Projekt\.github\workflows\ci.yml"));
        assert!(!prot(r"E:\Projekt\git\notes.txt"));
        assert!(!prot(r"E:\Projekt\.gitignore"));
        assert!(!prot(r"E:\Projekt\.gitconfig.bak"));
        assert!(!prot(r"E:\"));
    }
}
