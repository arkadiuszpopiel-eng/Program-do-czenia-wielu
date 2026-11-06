//! Ścieżki chronione w poleceniu (plan 50, przegląd 3.11): `Set-Content .git\hooks\pre-commit`
//! albo `> .npmrc` w katalogu roboczym. Analizator wyciąga tylko ścieżki bezwzględne i `..`, a
//! zakres samego katalogu roboczego ukryłby taki zapis — dlatego każde słowo polecenia, które
//! wygląda na ścieżkę chronioną, idzie do Sędziego Cieni jako osobny zakres. Filtr jest celowo
//! szeroki (lepiej jedno pytanie za dużo); rozstrzyga Sędzia z folderem pracy.

use compliance_contract::PathEnv;
use safety_broker_contract::{PathScope, is_protected_scope};
use tools_common_contract::paths;

/// Znaki, które w PowerShellu i `cmd` rozdzielają słowa (cudzysłowy, przekierowania, potoki).
const SEPARATORS: &[char] = &[
    ' ', '\t', '\r', '\n', '"', '\'', '`', ';', '|', '&', '<', '>', '(', ')', '{', '}', ',', '=',
];

/// Zakresy ścieżek chronionych wskazanych w poleceniu, rozwiązanych względem `cwd`.
pub(crate) fn scopes(command: &str, cwd: &str, env: &PathEnv) -> Vec<PathScope> {
    let mut out: Vec<PathScope> = Vec::new();
    for word in command.split(SEPARATORS).filter(|w| !w.is_empty()) {
        let word = word.trim_start_matches('@');
        let Ok(resolved) = paths::resolve_path_dots(word, Some(cwd), env) else {
            continue;
        };
        let Ok(scope) = paths::tree_scope(&resolved, env) else {
            continue;
        };
        if is_protected_scope(&scope) && !out.contains(&scope) {
            out.push(scope);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn found(cmd: &str) -> Vec<String> {
        let env = PathEnv::windows_profile(r"C:\Users\ala");
        scopes(cmd, r"C:\Users\ala\Projekt", &env)
            .iter()
            .map(|s| s.canonical().to_owned())
            .collect()
    }

    #[test]
    fn relative_protected_targets_are_found() {
        assert_eq!(
            found(r"Set-Content .git\hooks\pre-commit 'x'"),
            [r"c:\users\ala\projekt\.git\hooks\pre-commit"]
        );
        assert_eq!(found("echo x>.npmrc"), [r"c:\users\ala\projekt\.npmrc"]);
        assert_eq!(
            found(r#"Copy-Item a.txt ".vscode/settings.json""#),
            [r"c:\users\ala\projekt\.vscode\settings.json"]
        );
    }

    #[test]
    fn unix_style_workdir_like_the_harness() {
        let env = PathEnv::windows_profile("/Users/ala");
        let got = scopes(
            r"Set-Content .git\hooks\pre-commit 'x'",
            "/Users/ala/Projekt",
            &env,
        );
        assert_eq!(got.len(), 1, "{got:?}");
    }

    #[test]
    fn ordinary_commands_have_no_protected_targets() {
        assert!(found("cargo test -p a").is_empty());
        assert!(found(r"Copy-Item src\a.rs b.rs").is_empty());
        assert!(found("git status").is_empty());
    }
}
