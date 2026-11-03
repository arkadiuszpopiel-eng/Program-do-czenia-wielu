//! Ścieżki chronione (plan 50 §4.5, jak w Claude Code i Codex): repozytorium, konfiguracja
//! narzędzi i agentów, klucze, pliki startowe powłoki, Autostart. Zmiana takiej ścieżki nigdy nie
//! przechodzi bez pytania poza „Pełnym dostępem”; odczyt — bez zmian (sekrety i tak pilnują
//! deny-listy Jądra). Dopasowanie jak w deny-listach: bez wielkości liter, z aliasami 8.3 (`GIT~1`).

use compliance_contract::deny::{NormPath, comp_matches};

/// Katalogi chronione — gdziekolwiek w ścieżce (poniżej folderu pracy, patrz [`is_protected`]).
const DIRS: &[&str] = &[
    ".git",
    ".hg",
    ".alfa",
    ".codex",
    ".claude",
    ".agents",
    ".vscode",
    ".idea",
    ".husky",
    ".devcontainer",
    ".cargo",
    ".ssh",
    ".aws",
    ".azure",
    ".gnupg",
    ".kube",
    ".docker",
];

/// Katalogi z kluczami — chronione także wtedy, gdy folder pracy leży w nich samych.
const SECRET_DIRS: &[&str] = &[".ssh", ".aws", ".azure", ".gnupg", ".kube", ".docker"];

/// Ciągi katalogów chronione razem (przepływy CI, Autostart Windows).
const SEQUENCES: &[&[&str]] = &[
    &[".github", "workflows"],
    &["start menu", "programs", "startup"],
];

/// Pliki chronione — po nazwie (konfiguracja git i narzędzi, pliki startowe powłoki).
const FILES: &[&str] = &[
    ".gitconfig",
    ".gitmodules",
    ".gitattributes",
    ".npmrc",
    ".pypirc",
    ".netrc",
    ".envrc",
    ".bashrc",
    ".bash_profile",
    ".bash_logout",
    ".profile",
    ".zshrc",
    ".zshenv",
    ".zprofile",
    ".ripgreprc",
    ".mcp.json",
    ".claude.json",
    "profile.ps1",
    "microsoft.powershell_profile.ps1",
    "microsoft.vscode_powershell_profile.ps1",
];

/// Czy zakres zdolności dotyczy ścieżki chronionej (bez folderu pracy — wstępny filtr dla narzędzi,
/// np. ścieżek z polecenia powłoki; rozstrzyga Sędzia z folderem pracy).
pub fn is_protected_scope(scope: &crate::scope::PathScope) -> bool {
    is_protected(&scope.norm(), None)
}

/// Czy ścieżka jest chroniona względem folderu pracy (narzędzia sprawdzające zawartość katalogu
/// przed usunięciem; człony samego folderu pracy się nie liczą, jak u Sędziego).
pub fn is_protected_in(
    scope: &crate::scope::PathScope,
    workdir: Option<&crate::scope::PathScope>,
) -> bool {
    is_protected(
        &scope.norm(),
        workdir.map(crate::scope::PathScope::norm).as_ref(),
    )
}

fn any_of(comp: &str, names: &[&str]) -> bool {
    let comp = comp.to_lowercase();
    names.iter().any(|n| comp_matches(&comp, n))
}

/// Czy ścieżka jest chroniona. Człony samego folderu pracy (`workdir`, gdy ścieżka w nim leży)
/// się nie liczą — folder pracy w `.claude\worktrees` nie może pytać o każdy zapis — poza
/// katalogami z kluczami.
pub(crate) fn is_protected(p: &NormPath, workdir: Option<&NormPath>) -> bool {
    let Some(last) = p.comps.last() else {
        return false;
    };
    if any_of(last, FILES) {
        return true;
    }
    let base = workdir
        .filter(|w| {
            w.root == p.root
                && w.comps.len() <= p.comps.len()
                && w.comps
                    .iter()
                    .zip(&p.comps)
                    .all(|(a, b)| a.to_lowercase() == b.to_lowercase())
        })
        .map_or(0, |w| w.comps.len());
    let (above, below) = p.comps.split_at(base);
    if above.iter().any(|c| any_of(c, SECRET_DIRS)) || below.iter().any(|c| any_of(c, DIRS)) {
        return true;
    }
    SEQUENCES.iter().any(|seq| {
        below
            .windows(seq.len())
            .any(|w| w.iter().zip(seq.iter()).all(|(c, n)| any_of(c, &[n])))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use compliance_contract::deny::{PathEnv, normalize};

    fn norm(p: &str) -> NormPath {
        normalize(p, &PathEnv::new())
    }

    fn prot(p: &str) -> bool {
        is_protected(&norm(p), None)
    }

    fn prot_in(p: &str, wd: &str) -> bool {
        is_protected(&norm(p), Some(&norm(wd)))
    }

    #[test]
    fn protected_dirs_anywhere_in_path() {
        assert!(prot(r"E:\Projekt\.git"));
        assert!(prot(r"E:\Projekt\.git\config"));
        assert!(prot(r"E:\Projekt\.GIT\hooks\pre-commit"));
        assert!(prot(r"C:\Users\a\.ssh\id_ed25519"));
        assert!(prot(r"E:\Projekt\.vscode\settings.json"));
        assert!(prot(r"E:\Projekt\.cargo\config.toml"));
        assert!(prot(r"E:\Projekt\.github\workflows\ci.yml"));
        assert!(prot(
            r"C:\Users\a\AppData\Roaming\Microsoft\Windows\Start Menu\Programs\Startup\x.lnk"
        ));
    }

    #[test]
    fn protected_files_by_name() {
        assert!(prot(r"C:\Users\a\.gitconfig"));
        assert!(prot(r"E:\Projekt\.npmrc"));
        assert!(prot(r"E:\Projekt\.gitattributes"));
        assert!(prot(
            r"C:\Users\a\Documents\WindowsPowerShell\Microsoft.PowerShell_profile.ps1"
        ));
    }

    #[test]
    fn short_name_aliases_and_path_tricks_are_caught() {
        assert!(prot(r"E:\Projekt\GIT~1\hooks\pre-commit"));
        assert!(prot(r"C:\Users\a\SSH~1\id_rsa"));
        assert!(prot(r"C:\Users\a\GITCON~1"));
        assert!(prot(r"E:/Projekt/.git/config"));
        assert!(prot(r"\\?\E:\Projekt\.git\config"));
        assert!(prot(r"E:\Projekt\src\..\.git\config"));
        assert!(prot(r"E:\Projekt\.git.\config"));
    }

    #[test]
    fn ordinary_paths_are_not_protected() {
        assert!(!prot(r"E:\Projekt\src\main.rs"));
        assert!(!prot(r"E:\Projekt\.github\ISSUE_TEMPLATE\bug.md"));
        assert!(!prot(r"E:\Projekt\git\notes.txt"));
        assert!(!prot(r"E:\Projekt\.gitignore"));
        assert!(!prot(r"E:\Projekt\.gitconfig.bak"));
        assert!(!prot(r"E:\Projekt\startup\x.txt"));
        assert!(!prot(r"E:\"));
    }

    #[test]
    fn workdir_members_do_not_count_except_key_dirs() {
        let wd = r"E:\repo\.claude\worktrees\zadanie";
        assert!(!prot_in(r"E:\repo\.claude\worktrees\zadanie\src\a.rs", wd));
        assert!(prot_in(r"E:\repo\.claude\worktrees\zadanie\.git", wd));
        assert!(prot_in(r"E:\repo\.claude\settings.json", wd));
        assert!(prot_in(r"C:\Users\a\.ssh\x\a.txt", r"C:\Users\a\.ssh\x"));
        assert!(prot_in(r"E:\inne\.claude\x", r"E:\repo"));
    }
}
