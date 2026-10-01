//! `module.toml` modułu `voice-pipeline` jest poprawnym manifestem rejestru.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use core_registry_contract::ModuleManifest;

#[test]
fn module_manifest_parses() {
    let m = ModuleManifest::parse_toml(voice_pipeline_impl::MODULE_TOML).unwrap();
    assert_eq!(m.id.as_str(), "voice-pipeline");
    assert_eq!(m.provides[0].to_string(), "voice-pipeline-contract@1");
    assert!(
        m.requires
            .iter()
            .any(|r| r.to_string() == "voice-dialog-contract@1")
    );
}
