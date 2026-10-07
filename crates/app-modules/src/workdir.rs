//! Katalog roboczy agentek (zakres `tools-fs`/`tools-shell`): sprawdzenie przy wyborze i deny-listy
//! narzędzi z katalogami danych Alfy. Ścieżki sprawdzane w postaci podanej **i** po rozwiązaniu
//! dowiązań (symlink, junction, prefiks `\\?\`) — tą samą funkcją co narzędzia przy każdym
//! wywołaniu (`tools_common_contract::paths::protected_with_links`) i z normalizacją Windows
//! deny-list `compliance` (Q-1).

use std::path::Path;

use app_api::error::AppError;
use app_api::paths::AppPaths;
use compliance_contract::deny::{NormPath, comp_matches, normalize};
use compliance_contract::{DenyChecker, DenyLists, PathEnv};
use tools_common_contract::paths;

/// Katalogi danych Alfy (konfiguracja, dane lokalne) w postaci podanej i rozwiązanej.
pub fn protected_dirs(app: &AppPaths) -> Vec<String> {
    let mut out = Vec::new();
    for dir in [&app.local, &app.config] {
        let text = dir.to_string_lossy().into_owned();
        if let Ok(Some(real)) = paths::resolve_links(&text)
            && real != text
        {
            out.push(real);
        }
        out.push(text);
    }
    out
}

/// Deny-listy narzędzi agentek: lista bazowa Jądra + katalogi danych Alfy (narzędzia sprawdzają
/// je przy każdym wywołaniu, także po rozwiązaniu dowiązań — dowiązanie utworzone po wyborze
/// katalogu roboczego nie otwiera danych Alfy).
pub fn tool_deny_lists(app: &AppPaths) -> DenyLists {
    let mut lists = DenyLists::baseline();
    lists.path_prefixes.extend(protected_dirs(app));
    lists
}

/// Czy `a` leży w `b` (komponenty w normalizacji Windows, aliasy 8.3).
fn within(a: &NormPath, b: &NormPath) -> bool {
    a.root == b.root
        && b.comps.len() <= a.comps.len()
        && b.comps
            .iter()
            .zip(&a.comps)
            .all(|(p, c)| comp_matches(c, p))
}

/// Sprawdza katalog wybrany na zakres narzędzi: istniejący, bezwzględny, ani on, ani cel jego
/// dowiązań nie leży w danych Alfy / na deny-liście i nie zawiera danych Alfy. Zwraca ścieżkę
/// w postaci wybranej przez właściciela.
pub fn check_workdir(path: &Path, app: &AppPaths) -> Result<String, AppError> {
    if !path.is_absolute() || !path.is_dir() {
        return Err(AppError::invalid(format!(
            "„{}” nie jest istniejącym katalogiem.",
            path.display()
        )));
    }
    let text = path.to_string_lossy().into_owned();
    let (_, env): (String, PathEnv) = crate::broker::path_env_for(&app.user_root);
    let deny = DenyChecker::new(tool_deny_lists(app), &env);
    let internal: Vec<NormPath> = protected_dirs(app)
        .iter()
        .map(|d| normalize(d, &env))
        .collect();
    let hit = |p: &str| {
        let norm = normalize(p, &env);
        internal
            .iter()
            .any(|dir| within(&norm, dir) || within(dir, &norm))
            || deny.is_denied_normalized(&norm)
            || paths::has_credential_segment(p)
    };
    if paths::protected_with_links(&text, hit) {
        return Err(AppError::forbidden(format!(
            "„{text}” zawiera dane Alfy albo poświadczenia (także przez dowiązanie) — wybierz inny \
             katalog roboczy."
        )));
    }
    Ok(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn data_dirs_and_their_ancestors_are_rejected() {
        let dir = std::env::temp_dir().join(format!("alfa-workdir-{}", std::process::id()));
        let app = AppPaths::under(&dir);
        for d in [&app.local, &app.config, &app.user_root.join("Projekt")] {
            std::fs::create_dir_all(d).unwrap();
        }
        assert!(check_workdir(&app.user_root.join("Projekt"), &app).is_ok());
        for bad in [app.local.join("."), app.config.clone(), dir.clone()] {
            assert!(check_workdir(&bad, &app).is_err(), "{}", bad.display());
        }
        assert!(check_workdir(Path::new("wzgledny"), &app).is_err());
        let lists = tool_deny_lists(&app);
        assert!(lists.path_prefixes.len() >= DenyLists::baseline().path_prefixes.len() + 2);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
