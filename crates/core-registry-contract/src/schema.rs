//! JSON Schema manifestu — źródło dla `packages/schemas/module-manifest.v1.json`.

use crate::manifest::ModuleManifest;

/// Wersja schematu manifestu.
pub const MANIFEST_SCHEMA_VERSION: u32 = 1;

/// Schemat manifestu (schemars).
pub fn manifest_schema() -> schemars::Schema {
    let mut schema = schemars::schema_for!(ModuleManifest);
    if let Some(obj) = schema.as_object_mut() {
        obj.insert(
            "$id".into(),
            serde_json::Value::String(format!(
                "alfa://schemas/module-manifest.v{MANIFEST_SCHEMA_VERSION}.json"
            )),
        );
        obj.insert(
            "x-schema-version".into(),
            serde_json::Value::from(MANIFEST_SCHEMA_VERSION),
        );
    }
    schema
}

/// Schemat jako sformatowany JSON z końcowym znakiem nowej linii.
pub fn manifest_schema_json() -> String {
    let mut text = serde_json::to_string_pretty(&manifest_schema()).unwrap_or_default();
    text.push('\n');
    text
}
