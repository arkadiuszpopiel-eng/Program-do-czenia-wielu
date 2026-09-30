//! Współdzielone testy kontraktowe (feature `contract-tests`). Ten sam zestaw uruchamiają
//! `sessions-impl` i `sessions-fake`; rozjazd zachowań jest błędem testu.

mod catalog;
mod history;
pub mod ops;

use std::ops::Deref;

use crate::api::Sessions;

pub use catalog::{
    create_and_get, isolation_between_sessions, list_filters_and_sorts, tainted_only_grows,
    trash_restore_delete, unread_and_activity, update_meta, workdirs_are_unique,
};
pub use history::{
    append_and_project, append_rules, blocks_round_trip, drafts_per_session, fork_creates_variants,
    heard_prefix_is_append_only, hide_keeps_content, set_active_leaf_rules,
};

/// Uruchamia cały zestaw; `factory` daje świeżą, pustą instancję (np. w nowym katalogu tymczasowym).
pub fn run_all<H, S>(factory: impl Fn() -> H)
where
    H: Deref<Target = S>,
    S: Sessions,
{
    let cases: [fn(&dyn Sessions); 16] = [
        create_and_get,
        update_meta,
        tainted_only_grows,
        list_filters_and_sorts,
        unread_and_activity,
        trash_restore_delete,
        isolation_between_sessions,
        workdirs_are_unique,
        append_and_project,
        append_rules,
        fork_creates_variants,
        heard_prefix_is_append_only,
        hide_keeps_content,
        drafts_per_session,
        blocks_round_trip,
        set_active_leaf_rules,
    ];
    for case in cases {
        let harness = factory();
        case(&*harness);
    }
}

/// Skrót w testach: wynik albo panika z opisem błędu.
pub(crate) fn ok<T, E: std::fmt::Display>(r: Result<T, E>) -> T {
    r.unwrap_or_else(|e| panic!("nieoczekiwany błąd: {e}"))
}
