//! Specyfikacje komend serii 6 dla `dto_roundtrip.rs`: Arena (`turns_arena_answer`,
//! `turns_select_branch`), karty rozmów (`sessions_cards`), ustawienia dla firm
//! (`accounts_set_enterprise`), „Naucz Alfę” (`memory_teach*`), kanwa (`canvas_render`) i cennik
//! modeli (`pricing_*`, 0.0.66).

use app_core::dto::*;

use crate::{Check, roundtrip};

use super::Spec;

/// Specyfikacja komendy serii 6 (`None` — komenda spoza tej części).
pub fn spec(command: &str) -> Option<Spec> {
    let s: Check = roundtrip::<String>;
    Some(match command {
        "turns_arena_answer" => (vec![("sessionId", s), ("turnId", s), ("model", s)], s),
        "turns_select_branch" => (vec![("sessionId", s), ("turnId", s)], roundtrip::<()>),
        "sessions_cards" => (
            vec![("sessionIds", roundtrip::<Vec<String>>)],
            roundtrip::<Vec<SessionCard>>,
        ),
        "accounts_set_enterprise" => (
            vec![
                ("accountId", s),
                ("enterprise", roundtrip::<EnterpriseSettings>),
            ],
            roundtrip::<Account>,
        ),
        "memory_teach" => (
            vec![("request", roundtrip::<TeachRequest>)],
            roundtrip::<TeachReport>,
        ),
        "memory_teach_pick_file" => (vec![], roundtrip::<Option<TeachFile>>),
        "canvas_render" => (
            vec![
                ("kind", roundtrip::<CanvasKind>),
                ("source", s),
                ("lang", s),
            ],
            s,
        ),
        "pricing_get" => (vec![], roundtrip::<PricingView>),
        "pricing_apply" => (
            vec![("text", s), ("save", roundtrip::<bool>)],
            roundtrip::<PricingView>,
        ),
        _ => return None,
    })
}
