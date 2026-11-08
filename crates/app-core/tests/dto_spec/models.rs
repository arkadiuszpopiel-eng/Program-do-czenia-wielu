//! Specyfikacje komend menedżera modeli (`models_*`, pakiety `models_bundle*`,
//! `embed_model_activate`, `search_reindex_*`) dla `dto_roundtrip.rs`.

use app_core::dto::*;

use crate::{Check, roundtrip};

use super::Spec;

/// Specyfikacja komendy menedżera modeli (`None` — komenda spoza tej części).
pub fn spec(command: &str) -> Option<Spec> {
    let s: Check = roundtrip::<String>;
    let item: Check = roundtrip::<ModelItem>;
    let reindex: Check = roundtrip::<ReindexView>;
    Some(match command {
        "models_list" => (vec![], roundtrip::<ModelsView>),
        "models_download" | "models_cancel" | "models_verify" | "models_remove"
        | "models_repair" => (vec![("itemId", s)], item),
        "models_trust_hash" => (
            vec![("itemId", s), ("hashes", roundtrip::<TrustedHashes>)],
            item,
        ),
        "models_bundles" => (vec![], roundtrip::<Vec<ModelBundle>>),
        "models_bundle_download" | "models_bundle_verify" => {
            (vec![("bundleId", s)], roundtrip::<ModelBundle>)
        }
        "embed_model_activate" => (vec![("model", s)], roundtrip::<EmbedderView>),
        "search_reindex_start" | "search_reindex_cancel" | "search_reindex_status" => {
            (vec![], reindex)
        }
        _ => return None,
    })
}
