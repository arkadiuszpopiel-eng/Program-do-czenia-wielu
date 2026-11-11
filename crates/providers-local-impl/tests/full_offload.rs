//! Kontrakt `-ngl` przypiętego llama.cpp: bloki modelu oraz dodatkowy poziom wyjściowy.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use providers_local_impl::{BackendKey, LaunchPlan, builtin_models, layers_for};

fn plan(backend: BackendKey, gpu_layers: u32) -> LaunchPlan {
    LaunchPlan {
        program: "/server".into(),
        backend,
        gpu_layers,
        ctx: 8192,
        threads: 4,
        device: None,
    }
}

fn ngl(args: &[String]) -> u32 {
    let i = args.iter().position(|arg| arg == "-ngl").unwrap();
    args[i + 1].parse().unwrap()
}

#[test]
fn full_offload_includes_the_extra_llama_output_level() {
    let models = builtin_models().unwrap();
    for (id, expected) in [
        ("qwen3-8b-q4-k-m", 37),
        ("bielik-11b-v2.3-instruct-q4-k-m", 51),
        ("bielik-1.5b-v3.0-instruct-q8_0", 33),
    ] {
        let entry = models.iter().find(|e| e.id == id).unwrap();
        let logical_layers = layers_for(entry, entry.vram_need(8192), 8192);
        assert_eq!(logical_layers, entry.layers);
        for backend in [BackendKey::Cuda, BackendKey::Vulkan] {
            let args = plan(backend, logical_layers).args(
                entry,
                std::path::Path::new("/model.gguf"),
                1234,
                "fake-test-key",
            );
            assert_eq!(ngl(&args), expected, "{id}, {backend:?}");
        }
    }
}

#[test]
fn partial_offload_and_cpu_keep_their_exact_layer_limits() {
    let entry = builtin_models().unwrap().remove(0);
    for (backend, layers) in [
        (BackendKey::Cpu, 0),
        (BackendKey::Cuda, 0),
        (BackendKey::Cuda, 1),
        (BackendKey::Vulkan, entry.layers - 1),
    ] {
        let args = plan(backend, layers).args(
            &entry,
            std::path::Path::new("/model.gguf"),
            1234,
            "fake-test-key",
        );
        assert_eq!(ngl(&args), layers, "{backend:?}, {layers}");
    }
}
