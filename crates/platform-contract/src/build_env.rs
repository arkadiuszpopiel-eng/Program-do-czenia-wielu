//! Środowisko narzędzi programisty: zmienne budowania przepuszczane procesom potomnym
//! i domyślny limit równoległych zadań `cargo`.

/// Zmienne narzędzi budowania przepuszczane narzędziom programisty (bez sekretów): liczba zadań,
/// katalog wyników i tryb przyrostowy cargo, magazyny npm/pnpm.
pub const BUILD_ENV_ALLOWLIST: [&str; 6] = [
    "CARGO_BUILD_JOBS",
    "CARGO_TARGET_DIR",
    "CARGO_INCREMENTAL",
    "npm_config_cache",
    "npm_config_store_dir",
    "PNPM_HOME",
];

/// Domyślna liczba równoległych zadań budowania: połowa wątków procesora (1–8). Pełne `-j`
/// wyczerpywało pamięć i dysk tego PC przy `cargo test` (awaria 2026-10-17).
pub fn default_build_jobs(threads: usize) -> usize {
    (threads / 2).clamp(1, 8)
}

/// Uzupełnia środowisko o `CARGO_BUILD_JOBS` ([`default_build_jobs`]), gdy go nie podaje.
pub fn with_build_defaults(
    mut env: Vec<(String, String)>,
    threads: usize,
) -> Vec<(String, String)> {
    if !env
        .iter()
        .any(|(k, _)| k.eq_ignore_ascii_case("CARGO_BUILD_JOBS"))
    {
        env.push((
            "CARGO_BUILD_JOBS".into(),
            default_build_jobs(threads).to_string(),
        ));
    }
    env
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::exec::filter_env;

    /// Przegląd 2026-10-21: `cargo test` zlecony agentce szedł z pełnym `-j`.
    #[test]
    fn build_defaults_limit_jobs_unless_the_environment_sets_them() {
        assert_eq!(default_build_jobs(1), 1);
        assert_eq!(default_build_jobs(12), 6);
        assert_eq!(default_build_jobs(64), 8);
        let env = with_build_defaults(vec![("PATH".into(), "x".into())], 16);
        assert!(env.contains(&("CARGO_BUILD_JOBS".into(), "8".into())));
        let own = with_build_defaults(vec![("cargo_build_jobs".into(), "2".into())], 16);
        assert_eq!(own, vec![("cargo_build_jobs".to_owned(), "2".to_owned())]);
        // Zmienne budowania przechodzą przez filtr (nie wyglądają na sekrety).
        let kept = filter_env(
            [
                ("npm_config_store_dir", "D:/pnpm"),
                ("CARGO_TARGET_DIR", "D:/t"),
            ],
            &BUILD_ENV_ALLOWLIST,
        );
        assert_eq!(kept.len(), 2);
    }
}
