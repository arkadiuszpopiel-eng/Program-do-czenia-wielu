//! Przykładowe manifesty (testy kontraktowe, atrapa, UI w trybie demonstracyjnym).

use semver::Version;
use serde_json::json;

use crate::manifest::{PluginId, PluginLimits, PluginManifest, PluginToolDecl};
use crate::validate::sha256_hex;

/// Narzędzie „licznik słów” (`plugin_word_count`): `{text}` → `{words}`.
pub fn word_count_tool() -> PluginToolDecl {
    PluginToolDecl {
        name: "word_count".into(),
        title: "Licznik słów".into(),
        description: "Liczy słowa w podanym tekście (ciągi liter i cyfr, także polskich).".into(),
        input_schema: json!({
            "type": "object",
            "properties": { "text": { "type": "string", "description": "Tekst do policzenia." } },
            "required": ["text"],
            "additionalProperties": false
        }),
        output_schema: json!({
            "type": "object",
            "properties": { "words": { "type": "integer" } },
            "required": ["words"]
        }),
        mutating: false,
    }
}

/// Manifest wtyczki z jednym narzędziem, bez zdolności, dla podanych bajtów modułu.
pub fn manifest(id: &str, version: &str, wasm: &[u8]) -> PluginManifest {
    PluginManifest {
        id: PluginId::new(id),
        version: Version::parse(version).unwrap_or_else(|_| Version::new(0, 0, 1)),
        author: "Właściciel".into(),
        description: "Wtyczka przykładowa: licznik słów w piaskownicy Wasm.".into(),
        wasm_sha256: sha256_hex(wasm),
        capabilities: Vec::new(),
        limits: PluginLimits::default(),
        tools: vec![word_count_tool()],
    }
}
