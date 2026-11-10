//! Oczekiwana odmowa pamięci jest testem ochrony, nie zaliczoną generacją modelu.
use providers_contract::ProviderErrorKind;
use providers_local_impl::LocalProvider;
use serde_json::{Value, json};

pub fn record(
    provider: &LocalProvider,
    model: &str,
    error: Option<&ProviderErrorKind>,
    entry: &mut Value,
) -> bool {
    let budget = device_profile_contract::recommend(&super::setup::runner_profile())
        .residency
        .ram_mb;
    let Some(model) = provider.entry(model) else {
        return false;
    };
    let context = provider.sidecar().config().ctx.min(model.ctx);
    let required = model.ram_for(0, context);
    let guarded = required > budget
        && error == Some(&ProviderErrorKind::Unsupported)
        && entry["server"].is_null()
        && entry["text"].as_str() == Some("")
        && entry["output_tokens"].as_u64() == Some(0)
        && entry["error"]
            .as_str()
            .is_some_and(|s| s.contains("brak pamięci na model:"));
    entry["capacity"] = json!({"cpu_budget_mb":budget,"required_cpu_mb":required,"context":context,
        "expected_memory_rejection":guarded});
    entry["generation_status"] = json!(if guarded {
        "NOT_RUN_CPU_BUDGET"
    } else {
        "ATTEMPTED"
    });
    guarded
}

/// Sama poprawna odmowa pamięci nie wystarcza do zaliczenia próby silników.
pub fn require_generation(report: &mut super::Report) {
    let generated = report.value["llm"].as_array().is_some_and(|all| {
        all.iter().any(|e| {
            e["error"].is_null()
                && e["output_tokens"].as_u64().unwrap_or(0) >= 16
                && e["polish_letters"].as_bool() == Some(true)
        })
    });
    if !generated {
        report.problem("brak zaliczonej generacji LLM; odmowa pamięci nie jest generacją");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use app_api::AppPaths;
    use device_profile_contract::DeviceProfile;
    use device_profile_fake::FakeDeviceProfile;
    use futures_util::StreamExt;
    use model_residency_contract::Residency;
    use providers_contract::{
        CancellationToken, ChatRequest, Message, ModelProvider, ProviderEvent,
    };
    use providers_local_impl::{
        LaunchSpec, LocalError, Sidecar, SidecarLauncher, SidecarProcess, builtin_models, hash_path,
    };
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    struct NoLaunch(AtomicUsize);
    impl SidecarLauncher for NoLaunch {
        fn spawn(&self, _: &LaunchSpec) -> Result<Box<dyn SidecarProcess>, LocalError> {
            self.0.fetch_add(1, Ordering::SeqCst);
            Err(LocalError::Startup("controlled-no-process".into()))
        }
    }
    #[tokio::test]
    async fn actual_cpu_budget_guard_is_distinct_from_an_unexpected_startup_failure() {
        for (id, expected) in [
            ("bielik-11b-v2.3-instruct-q4-k-m", true),
            ("qwen3-8b-q4-k-m", false),
        ] {
            let dir = tempfile::tempdir().unwrap();
            let paths = AppPaths::under(dir.path());
            let models = builtin_models().unwrap();
            let model = models.iter().find(|m| m.id == id).unwrap();
            std::fs::create_dir_all(paths.models()).unwrap();
            let path = paths.models().join(&model.file);
            std::fs::write(&path, b"GGUF-controlled-fixture").unwrap();
            std::fs::write(hash_path(&path), &model.sha256).unwrap();
            let device: Arc<dyn DeviceProfile> =
                Arc::new(FakeDeviceProfile::new(super::super::setup::runner_profile()));
            assert_eq!(
                device.recommend().residency.ram_mb,
                8192,
                "budżet bez podnoszenia"
            );
            let residency = app_modules::route::local::residency(&device).unwrap();
            let launcher = Arc::new(NoLaunch(AtomicUsize::new(0)));
            let server = paths.sidecar("llama-cpu", "llama-server");
            std::fs::create_dir_all(server.parent().unwrap()).unwrap();
            std::fs::write(&server, b"controlled-never-executed").unwrap();
            let config = app_modules::route::local::local_config(&paths);
            let sidecar = Sidecar::new(
                config,
                launcher.clone(),
                Some(device),
                Some(residency.manager() as Arc<dyn Residency>),
            )
            .unwrap();
            let provider = LocalProvider::new(models, sidecar);
            let mut error = None;
            let mut text = String::new();
            let mut stream = provider.stream(
                ChatRequest::new(id, vec![Message::user_text("Test")]),
                CancellationToken::new(),
            );
            while let Some(e) = stream.next().await {
                match e {
                    ProviderEvent::Error(e) => error = Some(e),
                    ProviderEvent::TextDelta { text: t, .. } => text.push_str(&t),
                    _ => {}
                }
            }
            let mut entry = json!({"server":null,"text":text,"output_tokens":0,"error":error.as_ref().map(ToString::to_string)});
            assert_eq!(
                record(&provider, id, error.as_ref().map(|e| &e.kind), &mut entry),
                expected
            );
            assert_eq!(launcher.0.load(Ordering::SeqCst) == 0, expected);
            if expected {
                assert_eq!(entry["generation_status"], "NOT_RUN_CPU_BUDGET");
            }
        }
    }
}
