//! Silnik pobrany po starcie aplikacji (fala 6, próba generalna): plik `llama-server` jest
//! wybierany z kandydatów przy każdym starcie sidecara, nie raz przy składaniu aplikacji —
//! `llama-server` zainstalowany w Ustawieniach → Modele i silniki działa bez ponownego
//! uruchomienia Alfy (wcześniej pierwsza rozmowa z modelem lokalnym kończyła się błędem
//! „nie znaleziono pliku” aż do restartu).

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod support;

use futures_util::StreamExt;
use providers_contract::{CancellationToken, ChatRequest, Message, ModelProvider, ProviderEvent};
use providers_local_impl::{BackendKey, LocalConfig, LocalProvider};
use support::{Env, MODEL, fake_server};

async fn reply(p: &LocalProvider, text: &str) -> Vec<ProviderEvent> {
    let req = ChatRequest::new(MODEL, vec![Message::user_text(text)]);
    p.stream(req, CancellationToken::new()).collect().await
}

fn echo(events: &[ProviderEvent]) -> String {
    events
        .iter()
        .filter_map(|e| match e {
            ProviderEvent::TextDelta { text, .. } => Some(text.as_str()),
            _ => None,
        })
        .collect()
}

#[test]
fn first_existing_candidate_wins_at_call_time() {
    let dir = tempfile::tempdir().unwrap();
    let name = fake_server().file_name().unwrap().to_owned();
    let common = dir.path().join("llama").join(&name);
    let own = dir.path().join("llama-cuda").join(&name);
    let substitute = dir.path().join("llama-vulkan").join(&name);
    let mut config = LocalConfig::new("/m", &common);
    config
        .server_candidates
        .insert(BackendKey::Cuda, vec![own.clone(), substitute.clone()]);
    assert_eq!(
        config.server(BackendKey::Cuda),
        Some(common.clone()),
        "nic nie zainstalowano: ścieżka domyślna (błąd wskaże, gdzie umieścić serwer)"
    );
    for path in [&substitute, &own] {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, b"exe").unwrap();
        assert_eq!(config.server(BackendKey::Cuda), Some(path.clone()));
    }
    assert_eq!(
        config.server(BackendKey::Vulkan),
        Some(common),
        "backend bez kandydatów: server_bin"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn server_installed_after_start_works_without_restart() {
    let env = Env::new();
    let name = fake_server().file_name().unwrap().to_owned();
    let installed = env
        .dir
        .path()
        .join("sidecars")
        .join("llama-cpu")
        .join(&name);
    let mut config = env.config();
    let missing = env.dir.path().join("sidecars").join("llama").join(&name);
    for key in [BackendKey::Vulkan, BackendKey::Cuda, BackendKey::Cpu] {
        config.server_bin.insert(key, missing.clone());
        config
            .server_candidates
            .insert(key, vec![installed.clone(), missing.clone()]);
    }
    let p = env.provider_with(config, env.launcher("ok", None), None, None);
    let before = reply(&p, "przed instalacją").await;
    assert!(
        before.iter().any(|e| matches!(e, ProviderEvent::Error(_))),
        "brak pliku serwera → czytelny błąd: {before:?}"
    );
    std::fs::create_dir_all(installed.parent().unwrap()).unwrap();
    std::fs::copy(fake_server(), &installed).unwrap();
    let after = reply(&p, "po instalacji").await;
    assert_eq!(echo(&after), "Echo: po instalacji", "{after:?}");
    let (_, plan, _) = p.sidecar().running_plan().await.unwrap();
    assert_eq!(plan.program, installed);
    p.sidecar().stop("test").await;
}
