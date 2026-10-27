//! [`ProcessEngine`]: brak katalogu i brak programu bez uruchamiania czegokolwiek, a z
//! `ALFA_NEXTGO_ROOT` (katalog zbudowanego silnika) — test sprzętowy na prawdziwym serwerze:
//! scenariusz kontraktowy, dokumentacja, instalacja, kill-switch. Katalog silnika jest tylko
//! czytany; dane testu (zrzuty, katalog roboczy procesu) leżą w katalogu tymczasowym.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::PathBuf;
use std::sync::{Arc, RwLock};

use serde_json::json;
use tools_engine_contract::{EngineError, EngineMethod, EnginePort, StartOptions};
use tools_engine_impl::ProcessEngine;

fn engine(root: Option<PathBuf>, data: PathBuf) -> ProcessEngine {
    ProcessEngine::new(
        Arc::new(RwLock::new(root)),
        data,
        std::env::vars().collect(),
    )
}

#[tokio::test]
async fn missing_configuration_and_missing_binary_are_explained() {
    let d = tempfile::tempdir().unwrap();
    let none = engine(None, d.path().join("dane"));
    assert_eq!(
        none.start(StartOptions::default()).await,
        Err(EngineError::NotConfigured)
    );
    assert_eq!(none.docs().await, Err(EngineError::NotConfigured));
    assert!(!none.state().running);
    let empty = engine(Some(d.path().join("silnik")), d.path().join("dane"));
    assert!(matches!(
        empty.start(StartOptions::default()).await,
        Err(EngineError::NotInstalled(_))
    ));
    assert_eq!(
        empty
            .call(EngineMethod::HeadlessStep, json!({"frames": 1}))
            .await,
        Err(EngineError::NotRunning)
    );
    assert_eq!(empty.capture().await, Err(EngineError::NotRunning));
    assert_eq!(empty.kill_all(), 0);
}

fn real_root() -> Option<PathBuf> {
    let root = PathBuf::from(std::env::var_os("ALFA_NEXTGO_ROOT")?);
    root.join(tools_engine_contract::SERVER_EXE)
        .is_file()
        .then_some(root)
}

#[tokio::test]
async fn real_engine_passes_the_port_scenario() {
    let Some(root) = real_root() else {
        eprintln!("pominięte: ustaw ALFA_NEXTGO_ROOT na katalog zbudowanego silnika");
        return;
    };
    let d = tempfile::tempdir().unwrap();
    let e = engine(Some(root.clone()), d.path().to_path_buf());
    let install = e.installation();
    assert!(
        install.server.is_some() && install.version.is_some(),
        "{install:?}"
    );
    let docs = e.docs().await.unwrap();
    assert!(docs.iter().any(|doc| doc.name == "README.md"));
    assert!(e.read_doc("README.md").await.unwrap().contains('#'));

    tools_engine_contract::contract_tests::port_scenario(&e).await;

    // Kill-switch kończy działający serwer; kolejne wywołanie mówi, że nie działa.
    e.start(StartOptions::default()).await.unwrap();
    assert_eq!(e.kill_all(), 1);
    assert_eq!(
        e.call(EngineMethod::InspectWorldStreaming, json!({})).await,
        Err(EngineError::NotRunning)
    );
    // Nic nie powstało w katalogu silnika: wszystkie pliki procesu są w danych testu.
    assert!(!root.join("captures").exists() && !root.join("previews").exists());
}

/// Podgląd i zrzuty z okna wymagają karty graficznej i otwierają okno na ekranie — tylko
/// z `ALFA_NEXTGO_GPU=1`.
#[tokio::test]
async fn real_engine_renders_preview_and_window_captures() {
    let Some(root) = real_root().filter(|_| std::env::var_os("ALFA_NEXTGO_GPU").is_some()) else {
        eprintln!("pominięte: ALFA_NEXTGO_ROOT i ALFA_NEXTGO_GPU=1");
        return;
    };
    let d = tempfile::tempdir().unwrap();
    let e = engine(Some(root.clone()), d.path().to_path_buf());
    let shot = e
        .preview(tools_engine_contract::PreviewOptions {
            scene: tools_engine_contract::Scene::Demo,
            load: None,
            frames: 3,
            width: 320,
            height: 180,
        })
        .await
        .unwrap();
    assert!(shot.png.starts_with(d.path()), "podgląd w danych Alfy");
    assert!(std::fs::read(&shot.png).unwrap().starts_with(b"\x89PNG"));
    assert_eq!(shot.telemetry["width"], 320);

    let window = StartOptions {
        scene: tools_engine_contract::Scene::Demo,
        window: true,
        ..StartOptions::default()
    };
    let state = e.start(window).await.unwrap();
    assert_eq!(state.mode.as_deref(), Some("gpu_preview"));
    e.call(
        EngineMethod::ExecuteTransaction,
        json!({"transaction_id": "alfa-gpu-1", "operations": [{"op": "entity.spawn", "entity": {
            "tag": "alfa", "transform": {"position": [0.0, 2.0, 0.0],
            "rotation": [0.0, 0.0, 0.0, 1.0], "scale": [1.0, 1.0, 1.0]}}}]}),
    )
    .await
    .unwrap();
    let mut capture = None;
    for _ in 0..50 {
        capture = e.capture().await.unwrap();
        if capture.is_some() {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    }
    let capture = capture.expect("zrzut rewizji z okna");
    assert!(capture.png.starts_with(d.path()) && capture.revision.is_some());
    assert_eq!(e.stop().await, Ok(true));
}
