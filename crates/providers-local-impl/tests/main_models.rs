//! Role modeli i rzeczywisty rachunek wag + KV + rezerw dla karty 12 GiB.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use providers_local_impl::{BackendKey, LaunchPlan, LocalConfig, builtin_models};
mod support;

const MAIN: &str = "qwen3-8b-q4-k-m";
const POLISH: &str = "bielik-11b-v2.3-instruct-q4-k-m";
const GPU: u32 = 12 * 1024;
const DISPLAY: u32 = 768;
const SPEECH: u32 = 1500;
const TTS: u32 = 600;
const SPARE: u32 = 512;

#[test]
fn main_default_is_eight_billion_not_the_auxiliary_bielik() {
    assert_eq!(LocalConfig::new("/models", "/server").default_model, MAIN);
}

#[test]
fn both_main_models_keep_eight_k_context_and_real_reserves() {
    let models = builtin_models().unwrap();
    for (id, layers, kv) in [(MAIN, 36, 144), (POLISH, 50, 200)] {
        let entry = models.iter().find(|m| m.id == id).expect("model główny");
        assert!((8.0..=12.0).contains(&entry.params_b));
        assert_eq!((entry.layers, entry.kv_mb_per_1k_ctx), (layers, kv));
        assert!(entry.vram_need(8192) + DISPLAY + SPEECH + TTS + SPARE <= GPU);
        assert!(entry.url.contains("/resolve/") && entry.sha256.len() == 64);
    }
    let qwen = models.iter().find(|m| m.id == MAIN).unwrap();
    assert!(qwen.tools);
    assert!(qwen.vram_need(16384) + DISPLAY + SPEECH + TTS + SPARE <= GPU);
    let polish = models.iter().find(|m| m.id == POLISH).unwrap();
    let config = LocalConfig {
        ctx: 16384,
        ..LocalConfig::new("/models", "/server")
    };
    assert_eq!(
        config.ctx_for(polish, GPU - DISPLAY - TTS - SPARE, SPEECH),
        8192
    );
    assert!(
        !polish.tools,
        "nie deklarujemy niepotwierdzonego tool calling"
    );
}

#[test]
fn qwen_main_uses_non_thinking_template_to_keep_the_output_budget() {
    let mut entry = builtin_models().unwrap().remove(0);
    entry.id = MAIN.into();
    entry.tools = true;
    let plan = LaunchPlan {
        program: "/server".into(),
        backend: BackendKey::Cpu,
        gpu_layers: 0,
        ctx: 8192,
        threads: 2,
        device: None,
    };
    let args = plan.args(
        &entry,
        std::path::Path::new("/model.gguf"),
        1234,
        "fake-test-key",
    );
    let pos = args
        .iter()
        .position(|x| x == "--chat-template-kwargs")
        .expect("ustawienia Qwen3");
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&args[pos + 1]).unwrap(),
        serde_json::json!({"enable_thinking":false})
    );
    entry.id = "bielik-4.5b-v3.0-instruct-q8_0".into();
    assert!(
        !plan
            .args(
                &entry,
                std::path::Path::new("/model.gguf"),
                1234,
                "fake-test-key"
            )
            .iter()
            .any(|x| x == "--chat-template-kwargs")
    );
}

#[test]
fn auxiliary_bielik_stays_small_and_has_no_tool_capability() {
    let models = builtin_models().unwrap();
    let small = models
        .iter()
        .find(|m| m.id == "bielik-1.5b-v3.0-instruct-q8_0")
        .unwrap();
    assert!(!small.tools && small.params_b < 2.0);
    assert!(small.vram_need(4096) < 4096);
}

#[test]
fn main_route_never_silently_falls_back_to_the_installed_auxiliary() {
    use providers_contract::ModelProvider;
    use providers_local_impl::{LocalProvider, Sidecar};
    use std::sync::Arc;
    let env = support::Env::new();
    let mut models = builtin_models().unwrap();
    let small = models
        .iter()
        .find(|m| m.id == "bielik-1.5b-v3.0-instruct-q8_0")
        .unwrap();
    support::install(&env.models(), small);
    let mut config = env.config();
    config.default_model = MAIN.into();
    let sidecar = Sidecar::new(config, Arc::new(env.launcher("ok", None)), None, None).unwrap();
    let p = LocalProvider::new(models.clone(), sidecar);
    assert_eq!(
        p.capabilities().default_model,
        None,
        "pomocnik nie zastępuje modelu głównego"
    );
    let polish = models.iter().find(|m| m.id == POLISH).unwrap();
    support::install(&env.models(), polish);
    assert_eq!(p.capabilities().default_model.as_deref(), Some(POLISH));
    let main = models.iter().find(|m| m.id == MAIN).unwrap();
    support::install(&env.models(), main);
    assert_eq!(p.capabilities().default_model.as_deref(), Some(MAIN));
    let mut config = env.config();
    config.default_model = "bielik-1.5b-v3.0-instruct-q8_0".into();
    config.provider_id = "local-light".into();
    let sidecar = Sidecar::new(config, Arc::new(env.launcher("ok", None)), None, None).unwrap();
    let helper = LocalProvider::new(std::mem::take(&mut models), sidecar);
    assert_eq!(
        helper.capabilities().default_model.as_deref(),
        Some("bielik-1.5b-v3.0-instruct-q8_0")
    );
}
