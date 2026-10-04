//! „Zdrowie systemu" w aplikacji (kategoria `app-*`, wydzielona z `app-core` — limit rozmiaru
//! crate'a): Diagnosta (`diagnostician-impl`: sygnały z rejestru, logów Diagnostyki i watchdoga,
//! naprawy cofalne z dziennikiem), Ulepszacz (`improver-impl`: propozycje modelu lokalnego,
//! bramka z holdoutem, wdrożenie po zatwierdzeniu, rollback) i evale (`evals-impl`: katalog
//! z integralnością, `HoldoutGate`). Komendy `health_*`, `improver_*`, `evals_*` i zdarzenie
//! `HealthChanged`. Cykl Ulepszacza w bezczynności — tylko gdy kompozycja poda sygnały systemu
//! (port bezczynności/zasilania/trybu gry); inaczej wyłącznie ręcznie („Przeanalizuj teraz").

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

mod app;
mod diag;
mod improve;
mod symptom;
mod view;

pub use app::{HealthApp, HealthDeps, IDLE_CYCLE_EVERY};
pub use diag::{AUTONOMY_KEY, config_now, policy};
pub use improve::{
    LocalProposer, NoReplayRunner, SURFACE, UiDigestVerifier, evals_root, parse_candidates,
};
pub use improver_contract::RunConditions;
pub use symptom::symptom_tap;

/// Manifesty modułów składanych przez ten crate (identyfikator → `module.toml`).
pub const MODULES: &[(&str, &str)] = &[
    ("diagnostician", diagnostician_impl::MODULE_TOML),
    ("improver", improver_impl::MODULE_TOML),
    ("evals", evals_impl::MODULE_TOML),
];
