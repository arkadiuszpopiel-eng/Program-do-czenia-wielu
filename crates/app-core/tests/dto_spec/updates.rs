//! Specyfikacje komend aktualizacji i „O programie” (`updates_*`) dla `dto_roundtrip.rs`.

use app_core::dto::*;

use crate::{Check, roundtrip};

use super::Spec;

/// Specyfikacja komendy `updates_*` (`None` — komenda spoza tej części).
pub fn spec(command: &str) -> Option<Spec> {
    let view: Check = roundtrip::<UpdatesView>;
    let unit: Check = roundtrip::<()>;
    Some(match command {
        "updates_status" | "updates_check" | "updates_download" | "updates_cancel"
        | "updates_rollback" => (vec![], view),
        "updates_restart" | "updates_dismiss_whats_new" => (vec![], unit),
        "updates_about" => (vec![], roundtrip::<AboutInfo>),
        "updates_whats_new" => (vec![], roundtrip::<Option<WhatsNew>>),
        _ => return None,
    })
}
