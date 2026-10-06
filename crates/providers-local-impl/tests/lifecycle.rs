//! Cykl życia sidecara na prawdziwym procesie (fałszywy `llama-server`): start na żądanie,
//! `127.0.0.1` + losowy port i klucz per uruchomienie, argumenty z profilu urządzenia,
//! restart po awarii z limitem, fallback GPU → CPU, dzierżawy rezydencji, bezczynność, zdarzenia.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod support;

use std::sync::Arc;
use std::time::Duration;

use core_bus_fake::FakeBus;
use core_registry_contract::{HealthStatus, Module, ModuleContext, ModuleError};
use device_profile_contract::DeviceProfile;
use device_profile_fake::FakeDeviceProfile;
use futures_util::StreamExt;
use model_residency_contract::{LeaseRequest, ModelRole, Placement, Priority, Residency};
use model_residency_fake::FakeResidency;
use providers_contract::{
    CancellationToken, ChatRequest, HealthState, Message, ModelProvider, PrivacyTag,
    ProviderErrorKind, ProviderEvent, RequestPrivacy, StopReason,
};
use providers_local_impl::{LocalModule, LocalProvider, MODULE_TOML};
use support::{Env, MODEL, arg};
use tokio::time::Instant;

fn req(text: &str) -> ChatRequest {
    ChatRequest::new(MODEL, vec![Message::user_text(text)])
}

async fn run(p: &LocalProvider, r: ChatRequest) -> Vec<ProviderEvent> {
    p.stream(r, CancellationToken::new()).collect().await
}

fn text(events: &[ProviderEvent]) -> String {
    events
        .iter()
        .filter_map(|e| match e {
            ProviderEvent::TextDelta { text, .. } => Some(text.as_str()),
            _ => None,
        })
        .collect()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn starts_on_demand_on_localhost_with_random_port_and_key() {
    let env = Env::new();
    let p = env.provider("ok", None);
    assert!(env.launches().is_empty(), "bez żądania nie ma procesu");
    assert!(p.sidecar().running_plan().await.is_none());
    let events = run(&p, req("cześć")).await;
    assert_eq!(text(&events), "Echo: cześć");
    assert!(matches!(&events[0], ProviderEvent::Started { model, .. } if model == MODEL));
    assert_eq!(
        events.last(),
        Some(&ProviderEvent::stop(StopReason::EndTurn))
    );
    run(&p, req("drugi")).await;
    assert_eq!(
        env.launches().len(),
        1,
        "sidecar współdzielony między żądaniami"
    );
    p.sidecar().stop("test").await;
    run(&p, req("trzeci")).await;
    let launches = env.launches();
    assert_eq!(launches.len(), 2);
    for a in &launches {
        assert_eq!(arg(a, "--host").as_deref(), Some("127.0.0.1"));
        assert_eq!(arg(a, "--api-key").map(|k| k.len()), Some(48));
        assert_eq!(arg(a, "--alias").as_deref(), Some(MODEL));
        assert_eq!(arg(a, "-c").as_deref(), Some("8192"));
        assert_eq!(
            arg(a, "-ngl").as_deref(),
            Some("0"),
            "bez profilu urządzenia: CPU"
        );
        assert!(arg(a, "-m").unwrap().ends_with("test-4b.Q4_K_M.gguf"));
        assert!(a.contains(&"--jinja".to_owned()));
    }
    assert_ne!(arg(&launches[0], "--port"), arg(&launches[1], "--port"));
    assert_ne!(
        arg(&launches[0], "--api-key"),
        arg(&launches[1], "--api-key")
    );
    // Lokalny spełnia każdą jurysdykcję i sesję prywatną.
    let mut private = req("tajne");
    private.meta.privacy = RequestPrivacy {
        tag: PrivacyTag::Private,
        jurisdiction_allow: vec!["EU".into()],
    };
    assert_eq!(text(&run(&p, private).await), "Echo: tajne");
    assert_eq!(p.list_models().await.unwrap()[0].id, MODEL);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn crash_is_reported_and_restart_is_limited() {
    let env = Env::new();
    let mut config = env.config();
    config.max_restarts = 1;
    let p = env.provider_with(config, env.launcher("ok", None), None, None);
    let events = run(&p, req("CRASH")).await;
    match events.last() {
        Some(ProviderEvent::Error(e)) => assert!(e.after_output, "{e:?}"),
        other => panic!("{other:?}"),
    }
    assert_eq!(text(&run(&p, req("po awarii")).await), "Echo: po awarii");
    assert_eq!(env.launches().len(), 2, "restart po awarii");
    run(&p, req("CRASH")).await;
    let events = run(&p, req("znowu")).await;
    match events.last() {
        Some(ProviderEvent::Error(e)) => {
            assert!(e.message.contains("niestabilny"), "{e:?}");
            assert!(e.should_fallback(), "Router przełączy na inny cel");
        }
        other => panic!("{other:?}"),
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn gpu_start_failure_falls_back_to_cpu_within_5s() {
    let env = Env::new();
    let device: Arc<dyn DeviceProfile> = Arc::new(FakeDeviceProfile::desktop());
    let p = env.provider_with(
        env.config(),
        env.launcher("fail-gpu", None),
        Some(device),
        None,
    );
    let t0 = Instant::now();
    let events = run(&p, req("vulkan pada")).await;
    let took = t0.elapsed();
    assert_eq!(text(&events), "Echo: vulkan pada");
    eprintln!("fallback GPU → CPU: {took:?} (budżet 5 s)");
    assert!(
        took <= support::budget(Duration::from_secs(5)),
        "fallback CPU trwał {took:?}"
    );
    let launches = env.launches();
    assert_eq!(launches.len(), 2);
    assert_eq!(
        arg(&launches[0], "-ngl").as_deref(),
        Some("32"),
        "desktop 16 GB: całość na GPU"
    );
    assert_eq!(arg(&launches[1], "-ngl").as_deref(), Some("0"));
    let (_, plan, pid) = p.sidecar().running_plan().await.unwrap();
    assert_eq!(plan.backend.as_str(), "cpu");
    assert!(pid.is_some());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn device_profile_and_residency_drive_arguments() {
    let env = Env::new();
    let device = Arc::new(FakeDeviceProfile::baseline());
    let threads = device.current().cpu.physical_cores.to_string();
    let residency = Arc::new(FakeResidency::baseline());
    let p = env.provider_with(
        env.config(),
        env.launcher("ok", None),
        Some(device.clone() as Arc<dyn DeviceProfile>),
        Some(residency.clone() as Arc<dyn Residency>),
    );
    run(&p, req("gpu")).await;
    let first = env.launches().remove(0);
    assert_eq!(
        arg(&first, "-ngl").as_deref(),
        Some("32"),
        "dzierżawa GPU: pełne odciążenie"
    );
    assert_eq!(arg(&first, "--threads"), Some(threads));
    assert_eq!(
        residency.snapshot().used.vram_mb,
        3_000 + 512,
        "wagi + KV cache dla -c 8192"
    );
    // Głos (wyższy priorytet) potrzebuje VRAM → dzierżawa LLM odebrana, sidecar zatrzymany.
    let stt = LeaseRequest {
        owner: "voice-stt".into(),
        model: "whisper-large".into(),
        role: ModelRole::Stt,
        priority: Priority::VoiceRt,
        placement: Placement::GpuOnly,
        vram_mb: 6_000,
        ram_mb: 200,
        cpu_ram_mb: 0,
        idle_unload_ms: 0,
    };
    let grant = residency.acquire(stt).unwrap();
    assert_eq!(grant.evicted.len(), 1);
    // Wyładowanie odbywa się w zadaniu tła słuchacza dzierżawy — czekamy ograniczenie (≤ 5 s).
    let unloaded = tokio::time::timeout(Duration::from_secs(5), async {
        while p.sidecar().running_plan().await.is_some() {
            tokio::task::yield_now().await;
        }
    })
    .await;
    assert!(unloaded.is_ok(), "model wyładowany po eksmisji");
    // Następne żądanie: LLM nie mieści się obok głosu → CPU (-ngl 0).
    assert_eq!(text(&run(&p, req("cpu")).await), "Echo: cpu");
    assert_eq!(arg(&env.launches()[1], "-ngl").as_deref(), Some("0"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn idle_unload_module_events_and_health() {
    let env = Env::new();
    let p = Arc::new(env.provider("ok", None));
    let mut module = LocalModule::new(p.clone(), Duration::from_secs(3600)).unwrap();
    assert_eq!(module.health(), HealthStatus::NotStarted);
    let bus = FakeBus::default();
    module
        .start(ModuleContext::new(
            module.manifest().id.clone(),
            Arc::new(bus.clone()),
        ))
        .await
        .unwrap();
    assert_eq!(module.health(), HealthStatus::Healthy);
    run(&p, req("x")).await;
    let sidecar = p.sidecar();
    assert!(
        !sidecar.reap_idle_at(Instant::now()).await,
        "świeżo używany"
    );
    assert!(
        sidecar
            .reap_idle_at(Instant::now() + Duration::from_secs(601))
            .await
    );
    assert!(sidecar.running_plan().await.is_none());
    module.stop().await.unwrap();
    assert_eq!(module.stop().await, Err(ModuleError::NotStarted));
    for _ in 0..200 {
        if bus.recorded().len() >= 2 {
            break;
        }
        tokio::task::yield_now().await;
    }
    let names: Vec<String> = bus
        .recorded()
        .iter()
        .map(|e| e.kind.as_str().to_owned())
        .collect();
    assert_eq!(names, ["local.model.loaded", "local.model.unloaded"]);
    let loaded = &bus.recorded()[0].payload;
    assert_eq!(loaded["backend"], "cpu");
    assert!(
        !loaded.to_string().contains("api-key"),
        "bez klucza w zdarzeniach"
    );
    assert!(MODULE_TOML.contains("isolation = \"process\""));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn not_installed_unknown_model_and_cancel() {
    let env = Env::new();
    let p = env.provider(
        "ok",
        Some(r#"{"kind":"slow","chunks":["a","b","c","d","e","f"],"interval_ms":200}"#),
    );
    assert_eq!(p.health().state, HealthState::Healthy);
    let events = run(
        &p,
        ChatRequest::new("inny-model", vec![Message::user_text("x")]),
    )
    .await;
    assert!(
        matches!(&events[..], [ProviderEvent::Error(e)] if e.kind == ProviderErrorKind::InvalidRequest)
    );
    let cancel = CancellationToken::new();
    let mut s = p.stream(req("x"), cancel.clone());
    while let Some(ev) = s.next().await {
        if ev.is_content() {
            break;
        }
    }
    let t = Instant::now();
    cancel.cancel();
    let rest: Vec<_> = s.collect().await;
    let took = t.elapsed();
    eprintln!("anulowanie lokalne: {took:?} (budżet 100 ms)");
    assert!(
        took <= support::budget(Duration::from_millis(100)),
        "{took:?}"
    );
    assert_eq!(
        rest.last(),
        Some(&ProviderEvent::stop(StopReason::Cancelled))
    );
    // Model niepobrany: dostawca „nieskonfigurowany", błąd z fallbackiem.
    std::fs::remove_file(env.models().join("test-4b.Q4_K_M.gguf")).unwrap();
    assert_eq!(p.health().state, HealthState::Unconfigured);
    assert_eq!(p.capabilities().default_model, None);
    let events = run(&p, req("x")).await;
    match &events[..] {
        [ProviderEvent::Error(e)] => {
            assert_eq!(e.kind, ProviderErrorKind::Unsupported);
            assert!(e.should_fallback());
        }
        other => panic!("{other:?}"),
    }
    assert!(
        p.cost(MODEL, &providers_contract::Usage::default())
            .is_some()
    );
}
