//! Działania na pakietach na lokalnym serwerze HTTP: pobranie pakietu (limit równoległości),
//! weryfikacja z wykryciem uszkodzenia, naprawa uszkodzonej pozycji przy ponownym pobraniu,
//! zatrzymanie na zgodzie TOFU, pomijanie pozycji instalowanych ręcznie, `models_repair`
//! (wstrzymane pobieranie od zera, odrzucenia z czytelnym komunikatem).

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use app_api::ErrorCode;
use app_api::dto::{
    BundleState as B, ModelBundle, ModelItemKind as K, ModelItemState as S, TrustedHashes,
};
use app_models::Machine;
use app_models::bundle_data::{LLAMA_CPU, LLM_SMALL, STT_SMALL, VAD, WHISPER_CPU};
use app_models::catalog::{Install, ItemSpec};
use common::http::Server;
use common::{Harness, blob, harness, options, spec};

const MIB: usize = 1024 * 1024;

/// Bez karty, 8 GB RAM, 4 rdzenie — silniki CPU, zalecany pakiet 2.
fn cpu_machine() -> Machine {
    Machine {
        gpu: None,
        vram_mb: 0,
        ram_mb: 8_192,
        cpu_cores: 4,
    }
}

fn manual(server: &Server, id: &str) -> ItemSpec {
    spec(
        id,
        K::Sidecar,
        server,
        &[],
        Install::Manual(vec!["whisper-server".into()]),
    )
}

async fn bundle(h: &Harness, id: &str) -> ModelBundle {
    let all = h.app.bundles(&cpu_machine()).await.unwrap();
    assert_eq!(all.len(), 6);
    assert_eq!(all.iter().filter(|b| b.recommended).count(), 1);
    all.into_iter().find(|b| b.id == id).unwrap()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn minimal_bundle_downloads_verifies_and_repairs_a_damaged_item() {
    let server = Server::start().await;
    let (llm, llama) = (blob(2 * MIB, 1), blob(MIB, 2));
    let catalog = vec![
        spec(
            LLM_SMALL,
            K::Llm,
            &server,
            &[("s.gguf", "/s.gguf", &llm, true)],
            Install::Files,
        ),
        spec(
            LLAMA_CPU,
            K::Sidecar,
            &server,
            &[("srv.bin", "/srv.bin", &llama, true)],
            Install::Files,
        ),
    ];
    let h = harness(catalog, None, options(1));
    let m = cpu_machine();
    let fresh = bundle(&h, "bundle-minimal").await;
    let size = (3 * MIB) as u64;
    assert_eq!(
        (fresh.state, fresh.total, fresh.missing_bytes),
        (B::NotInstalled, 2, size)
    );
    assert!(bundle(&h, "bundle-light").await.recommended);

    let started = h.app.bundle_download("bundle-minimal", &m).await.unwrap();
    assert_eq!(started.state, B::Downloading);
    h.wait(LLM_SMALL, S::Installed).await;
    h.wait(LLAMA_CPU, S::Installed).await;
    let done = bundle(&h, "bundle-minimal").await;
    assert_eq!(
        (done.state, done.installed, done.missing_bytes),
        (B::Installed, 2, 0)
    );
    let requests = server.requests().len();
    // Zainstalowany pakiet: ponowne „pobierz” niczego nie pobiera.
    let again = h.app.bundle_download("bundle-minimal", &m).await.unwrap();
    assert_eq!(
        (again.state, server.requests().len()),
        (B::Installed, requests)
    );
    let verified = h.app.bundle_verify("bundle-minimal", &m).await.unwrap();
    assert_eq!(verified.state, B::Installed);

    let file = h.paths.models().join(format!("t/{LLAMA_CPU}/srv.bin"));
    std::fs::write(&file, b"uszkodzony plik").unwrap();
    let damaged = h.app.bundle_verify("bundle-minimal", &m).await.unwrap();
    assert_eq!(damaged.state, B::Corrupt);
    let row = damaged.items.iter().find(|i| i.id == LLAMA_CPU).unwrap();
    assert_eq!((row.state, row.fallback), (S::Corrupt, false));
    assert_eq!(damaged.missing_bytes, MIB as u64);

    let repairing = h.app.bundle_download("bundle-minimal", &m).await.unwrap();
    assert_eq!(repairing.state, B::Downloading);
    h.wait(LLAMA_CPU, S::Installed).await;
    assert_eq!(std::fs::read(&file).unwrap(), llama);
    assert_eq!(bundle(&h, "bundle-minimal").await.state, B::Installed);
    assert_eq!(
        h.item(LLM_SMALL).await.state,
        S::Installed,
        "zdrowa pozycja nietknięta"
    );

    for result in [
        h.app.bundle_download("bundle-nie-ma", &m).await,
        h.app.bundle_verify("bundle-nie-ma", &m).await,
    ] {
        let err = result.unwrap_err();
        assert_eq!(err.code, ErrorCode::NotFound);
        assert!(
            err.message.contains("Nie ma pakietu „bundle-nie-ma”"),
            "{}",
            err.message
        );
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn light_bundle_stops_at_trust_and_skips_manual_items() {
    let server = Server::start().await;
    let (llm, llama, stt, vad) = (blob(MIB, 3), blob(MIB, 4), blob(MIB, 5), blob(MIB, 6));
    let catalog = vec![
        spec(
            LLM_SMALL,
            K::Llm,
            &server,
            &[("s.gguf", "/s.gguf", &llm, true)],
            Install::Files,
        ),
        spec(
            LLAMA_CPU,
            K::Sidecar,
            &server,
            &[("srv.bin", "/srv.bin", &llama, true)],
            Install::Files,
        ),
        spec(
            STT_SMALL,
            K::Stt,
            &server,
            &[("ggml-small.bin", "/stt.bin", &stt, false)],
            Install::Files,
        ),
        spec(
            VAD,
            K::Vad,
            &server,
            &[("vad.onnx", "/vad.onnx", &vad, true)],
            Install::Files,
        ),
        manual(&server, WHISPER_CPU),
    ];
    let h = harness(catalog, None, options(2));
    let m = cpu_machine();
    let started = h.app.bundle_download("bundle-light", &m).await.unwrap();
    assert_eq!((started.state, started.total), (B::Downloading, 5));
    let pending = h.wait(STT_SMALL, S::NeedsTrust).await;
    for id in [LLM_SMALL, LLAMA_CPU, VAD] {
        h.wait(id, S::Installed).await;
    }
    let waiting = bundle(&h, "bundle-light").await;
    assert_eq!(
        (waiting.state, waiting.installed, waiting.missing_bytes),
        (B::NeedsTrust, 3, 0)
    );
    let engine = waiting.items.iter().find(|i| i.id == WHISPER_CPU).unwrap();
    assert_eq!((engine.state, engine.downloadable), (S::Missing, false));
    let requests = server.requests().len();
    h.app.bundle_download("bundle-light", &m).await.unwrap();
    assert_eq!(server.requests().len(), requests, "bez ponownego pobrania");
    assert_eq!(h.item(STT_SMALL).await.state, S::NeedsTrust);

    let hashes: TrustedHashes = pending
        .files
        .iter()
        .map(|f| (f.name.clone(), f.sha256.clone().unwrap()))
        .collect();
    h.app.trust_hash(STT_SMALL, hashes).await.unwrap();
    h.wait(STT_SMALL, S::Installed).await;
    let partial = bundle(&h, "bundle-light").await;
    assert_eq!(
        (partial.state, partial.installed, partial.total),
        (B::Partial, 4, 5)
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn repair_restarts_from_scratch_and_rejects_unknown_or_manual_items() {
    let server = Server::start().await;
    let llm = blob(2 * MIB, 7);
    let catalog = vec![
        spec(
            LLM_SMALL,
            K::Llm,
            &server,
            &[("s.gguf", "/s.gguf", &llm, true)],
            Install::Files,
        ),
        manual(&server, WHISPER_CPU),
    ];
    let h = harness(catalog, None, options(2));
    server.with(|s| s.stall_next_after = Some(300_000));
    h.app.download(LLM_SMALL).await.unwrap();
    let part = h
        .paths
        .local
        .join(format!("downloads/{LLM_SMALL}/s.gguf.part"));
    for _ in 0..500 {
        if std::fs::metadata(&part).is_ok_and(|m| m.len() >= 300_000) {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    assert_eq!(h.app.cancel(LLM_SMALL).await.unwrap().state, S::Paused);

    let repaired = h.app.repair(LLM_SMALL).await.unwrap();
    assert!(
        matches!(repaired.state, S::Queued | S::Downloading),
        "{repaired:?}"
    );
    h.wait(LLM_SMALL, S::Installed).await;
    let ranges: Vec<_> = server
        .requests()
        .into_iter()
        .filter_map(|(_, r)| r)
        .collect();
    assert!(ranges.is_empty(), "naprawa zaczyna od zera: {ranges:?}");

    // Naprawa zainstalowanej pozycji pobiera ją jeszcze raz.
    let before = server.requests().len();
    h.app.repair(LLM_SMALL).await.unwrap();
    h.wait(LLM_SMALL, S::Installed).await;
    assert_eq!(server.requests().len(), before + 1);
    let file = h.paths.models().join(format!("t/{LLM_SMALL}/s.gguf"));
    assert_eq!(std::fs::read(file).unwrap(), llm);

    let unknown = h.app.repair("nie-ma").await.unwrap_err();
    assert_eq!(unknown.code, ErrorCode::NotFound);
    assert!(
        unknown.message.contains("Nie ma pozycji „nie-ma”"),
        "{}",
        unknown.message
    );
    let manual = h.app.repair(WHISPER_CPU).await.unwrap_err();
    assert_eq!(manual.code, ErrorCode::InvalidInput);
    assert!(
        manual.message.contains("instaluje się ręcznie"),
        "{}",
        manual.message
    );
}
