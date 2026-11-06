//! Specyfikacja komendy profilu Dziecko (`permissions_child_lock`, plan 50 §4.4) dla
//! `dto_roundtrip.rs`: PIN tylko wchodzi (bez serializacji), stan wraca bez PIN-u.

use app_core::dto::*;

use crate::{parse_only, roundtrip};

use super::Spec;

/// Specyfikacja komendy profilu Dziecko (`None` — komenda spoza tej części).
pub fn spec(command: &str) -> Option<Spec> {
    Some(match command {
        "permissions_child_lock" => (
            vec![("change", parse_only::<Option<ChildLockRequest>>)],
            roundtrip::<ChildLockState>,
        ),
        _ => return None,
    })
}
