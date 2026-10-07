//! Przegląd bezpieczeństwa #2 (docs/reviews/2026-10-security-review-2.md) — testy regresyjne:
//! szczegóły sygnału (`diagnostics.symptom` z magistrali — moduł nadawcy nie jest uwierzytelniony)
//! są niezaufane; nie wybierają dowolnego klucza konfiguracji ani dowolnej wartości naprawy.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeMap;
use std::sync::Arc;

use diagnostician_contract::contract_tests::{BrokerMode, MiniWorld, ScriptedBroker};
use diagnostician_contract::{Diagnostician, RepairAutonomy, RepairPolicy, Signal, Symptom};
use diagnostician_fake::fake_diagnostician;
use serde_json::json;
use watchdog_contract::ManualClock;

fn signal(module: &str, symptom: Symptom, details: &[(&str, &str)]) -> Signal {
    Signal::Error {
        module: module.into(),
        symptom,
        target: Some(module.into()),
        details: details
            .iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect::<BTreeMap<_, _>>(),
    }
}

async fn repair(config: &[(&str, serde_json::Value)], s: Signal) -> Arc<MiniWorld> {
    let world = MiniWorld::new(config, &[]);
    let broker = ScriptedBroker::new(world.clone(), BrokerMode::Approve);
    let policy = RepairPolicy {
        autonomy: RepairAutonomy::AutoMediumRisk,
        ..RepairPolicy::default()
    };
    let clock = Arc::new(ManualClock::new(1_000));
    let (_host, diag) = fake_diagnostician(clock, world.clone(), world.clone(), broker, policy);
    diag.ingest(s).await;
    diag.scan().await;
    world
}

/// SR2-03: klucz z `device_key` sygnału „utracono GPU” był zapisywany bez względu na moduł
/// i rodzaj ustawienia — sfałszowany symptom przekierowywał dostawcę (`base_url`), podmieniał
/// polecenie serwera MCP albo prompt roli, wyłączał watchdoga (naprawa niskiego ryzyka = auto).
#[tokio::test]
async fn signal_cannot_choose_arbitrary_config_key() {
    for key in [
        "providers.openai.base_url",
        "mcp.servers.evil.command",
        "roles.conductor.prompt",
        "modules.watchdog.enabled",
        "broker.pipe_name",
        "watchdog.max_restarts",
        "updater.channel",
        "egress.allow",
        "net.proxy",
        "budgets.monthly_pln",
        "tools.shell.allow",
        "voice.stt.base_url",
    ] {
        let world = repair(
            &[(key, json!("stare")), ("voice.stt.device", json!("vulkan"))],
            signal(
                "voice-stt",
                Symptom::GpuDeviceLost,
                &[("device_key", key), ("fallback", "https://evil.example/v1")],
            ),
        )
        .await;
        assert_eq!(
            world.snapshot().config[key],
            json!("stare"),
            "{key}: Diagnosta nie może zapisać klucza wskazanego przez sygnał"
        );
    }
}

/// SR2-03: wartość zastępcza z sygnału (adres, ścieżka) nie trafia do konfiguracji.
#[tokio::test]
async fn signal_fallback_value_is_a_plain_token() {
    let world = repair(
        &[("voice.stt.device", json!("vulkan"))],
        signal(
            "voice-stt",
            Symptom::GpuDeviceLost,
            &[
                ("device_key", "voice.stt.device"),
                ("fallback", "https://evil.example/v1"),
            ],
        ),
    )
    .await;
    let v = world.snapshot().config["voice.stt.device"].clone();
    assert_ne!(v, json!("https://evil.example/v1"));
    let world = repair(
        &[("voice.stt.device", json!("vulkan"))],
        signal(
            "voice-stt",
            Symptom::GpuDeviceLost,
            &[("device_key", "voice.stt.device"), ("fallback", "directml")],
        ),
    )
    .await;
    assert_eq!(
        world.snapshot().config["voice.stt.device"],
        json!("directml"),
        "poprawny klucz modułu i zwykła wartość — naprawa działa"
    );
}

/// P2-09: „ostatnia dobra rewizja” przywracała **całą** konfigurację — także klucze zakazane
/// Diagnoście (prywatność, autonomia, budżety), które właściciel zmienił po tej rewizji. Awaria
/// modułu (tu: zły silnik TTS) cofała zaostrzenie prywatności i podniesiony próg budżetu.
#[tokio::test]
async fn rollback_never_restores_keys_forbidden_to_diagnostician() {
    use diagnostician_contract::{ModuleCondition, RepairStep};
    use diagnostician_fake::{ChaosSetup, ChaosWorld, chaos_diagnostician};

    let clock = Arc::new(ManualClock::new(1_000));
    let world = ChaosWorld::baseline(clock.clone());
    world.mutate(|st| {
        // Ostatnia dobra rewizja: luźniejsze ustawienia bezpieczeństwa.
        st.config
            .insert("privacy.cloud_allowed".into(), json!(true));
        st.config.insert("autonomy.level".into(), json!("L4"));
        st.config.insert("budgets.monthly_pln".into(), json!(500));
        st.revisions.insert("r1".into(), st.config.clone());
        // Właściciel zaostrza; równocześnie psuje się moduł TTS.
        st.config
            .insert("privacy.cloud_allowed".into(), json!(false));
        st.config.insert("autonomy.level".into(), json!("L2"));
        st.config.insert("budgets.monthly_pln".into(), json!(50));
        st.config
            .insert("voice.tts.engine".into(), json!("nieznany"));
        st.revisions.insert("r2".into(), st.config.clone());
        st.revision = "r2".into();
    });
    let policy = RepairPolicy {
        autonomy: RepairAutonomy::AutoMediumRisk,
        ..RepairPolicy::default()
    };
    let (_host, diag) = chaos_diagnostician(ChaosSetup {
        world: world.clone(),
        clock,
        policy,
    });
    diag.ingest(Signal::ModuleState {
        module: "voice-tts".into(),
        condition: ModuleCondition::Failed { restarts: 3 },
        detail: "nieznany silnik".into(),
    })
    .await;
    diag.scan().await;
    let config = world.snapshot().config;
    assert_eq!(config["privacy.cloud_allowed"], json!(false), "prywatność");
    assert_eq!(config["autonomy.level"], json!("L2"), "autonomia");
    assert_eq!(config["budgets.monthly_pln"], json!(50), "budżet");
    assert_ne!(
        config.get("voice.tts.engine"),
        Some(&json!("nieznany")),
        "dozwolony klucz modułu naprawiony"
    );
    assert!(
        world
            .env_log()
            .iter()
            .all(|s| !matches!(s, RepairStep::RollbackConfig { .. })),
        "bez przywracania całej rewizji"
    );
}
