//! Specyfikacje komend wtyczek Wasm (`plugins_*`) dla `dto_roundtrip.rs`.

use app_core::dto::*;

use crate::{Check, roundtrip};

use super::Spec;

/// Specyfikacja komendy `plugins_*` (`None` — komenda spoza tej części).
pub fn spec(command: &str) -> Option<Spec> {
    let s: Check = roundtrip::<String>;
    let info: Check = roundtrip::<PluginInfo>;
    let view: Check = roundtrip::<PluginsView>;
    Some(match command {
        "plugins_list" => (vec![], view),
        "plugins_inspect" => (vec![("wasmB64", s)], roundtrip::<PluginInspection>),
        "plugins_propose" => (
            vec![("manifest", roundtrip::<serde_json::Value>), ("wasmB64", s)],
            info,
        ),
        "plugins_approve" => (
            vec![("pluginId", s), ("version", s), ("reviewedHash", s)],
            info,
        ),
        "plugins_reject" => (vec![("pluginId", s), ("version", s)], info),
        "plugins_disable" => (vec![("pluginId", s)], info),
        "plugins_enable" => (vec![("pluginId", s), ("reviewedHash", s)], info),
        "plugins_remove" => (vec![("pluginId", s)], view),
        _ => return None,
    })
}
