//! Konfiguracja ręcznej próby; dodatkowy model tylko z publicznego, przypiętego manifestu.
use providers_local_impl::{LocalConfig, ModelEntry};

pub(super) fn configuration() -> (Vec<ModelEntry>, LocalConfig, String) {
    let server = std::env::var("ALFA_LIVE_SERVER").expect("ALFA_LIVE_SERVER");
    let models_dir = std::env::var("ALFA_LIVE_MODELS_DIR").expect("ALFA_LIVE_MODELS_DIR");
    let model = std::env::var("ALFA_LIVE_MODEL").expect("ALFA_LIVE_MODEL");
    let mut models = providers_local_impl::builtin_models().unwrap();
    if let Ok(path) = std::env::var("ALFA_LIVE_MODEL_ENTRY") {
        let entry: ModelEntry = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        entry.validate().unwrap();
        assert!(
            !entry.sha256.is_empty(),
            "próba dodatkowego modelu wymaga SHA256"
        );
        assert!(
            models.iter().all(|m| m.id != entry.id),
            "manifest próby nie może zmieniać wbudowanych modeli ani ich zdolności"
        );
        models.push(entry);
    }
    assert!(models.iter().any(|m| m.id == model));
    let mut config = LocalConfig::new(models_dir, server);
    config.default_model.clone_from(&model);
    config.threads = Some(4);
    (models, config, model)
}
