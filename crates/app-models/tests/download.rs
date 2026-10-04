//! Pobieranie i instalacja na lokalnym serwerze HTTP: przypięty hash, wznowienie po zerwaniu
//! i po anulowaniu (`Range`), zły hash = usunięcie pliku, zgoda TOFU (zły/zmieniony hash
//! odrzucony), limit równoległości, limit rozmiaru, tylko HTTPS, GGUF z rekordem `providers-local`,
//! archiwum sidecara, weryfikacja i usuwanie, zdarzenia postępu `{file, done, total}`.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use app_api::dto::{AlfaEvent, ModelItemKind as K, ModelItemState as S, TrustedHashes};
use app_models::catalog::Install;
use common::http::Server;
use common::{blob, harness, options, sha, spec};

const MIB: usize = 1024 * 1024;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn pinned_file_downloads_installs_and_reports_progress() {
    let server = Server::start().await;
    let data = blob(3 * MIB, 1);
    let item = spec(
        "stt",
        K::Stt,
        &server,
        &[("ggml-x.bin", "/x.bin", &data, true)],
        Install::Files,
    );
    let h = harness(vec![item], None, options(2));
    assert_eq!(h.item("stt").await.state, S::Missing);
    h.app.download("stt").await.unwrap();
    let done = h.wait("stt", S::Installed).await;
    assert!(done.pinned && done.files[0].sha256.as_deref() == Some(sha(&data).as_str()));
    let path = h.paths.models().join("t/stt/ggml-x.bin");
    assert_eq!(std::fs::read(&path).unwrap(), data);
    assert!(
        !h.paths.local.join("downloads/stt").exists(),
        "katalog roboczy sprzątnięty"
    );
    let events = h.events();
    let progress: Vec<_> = events
        .iter()
        .filter_map(|e| match e {
            AlfaEvent::ModelProgress {
                item_id,
                file,
                done,
                total,
            } if item_id == "stt" => Some((file.clone(), *done, *total)),
            _ => None,
        })
        .collect();
    assert!(!progress.is_empty());
    assert!(
        progress
            .iter()
            .all(|(f, _, t)| f == "ggml-x.bin" && *t == Some(data.len() as u64))
    );
    assert_eq!(progress.last().map(|p| p.1), Some(data.len() as u64));
    let states: Vec<S> = events
        .iter()
        .filter_map(|e| match e {
            AlfaEvent::ModelChanged { item } => Some(item.state),
            _ => None,
        })
        .collect();
    for s in [S::Queued, S::Downloading, S::Installing, S::Installed] {
        assert!(states.contains(&s), "{s:?} w {states:?}");
    }
    // Ponowne „pobierz” na zainstalowanej pozycji nic nie robi.
    assert_eq!(h.app.download("stt").await.unwrap().state, S::Installed);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn broken_stream_resumes_with_range_header() {
    let server = Server::start().await;
    let data = blob(2 * MIB, 2);
    let item = spec(
        "vad",
        K::Vad,
        &server,
        &[("v.onnx", "/v.onnx", &data, true)],
        Install::Files,
    );
    let h = harness(vec![item], None, options(2));
    server.with(|s| s.cut_next_after = Some(700_000));
    h.app.download("vad").await.unwrap();
    h.wait("vad", S::Installed).await;
    let ranges: Vec<_> = server
        .requests()
        .into_iter()
        .filter_map(|(_, r)| r)
        .collect();
    assert_eq!(ranges, ["bytes=700000-"]);
    assert_eq!(
        std::fs::read(h.paths.models().join("t/vad/v.onnx")).unwrap(),
        data
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cancel_keeps_partial_and_download_resumes_it() {
    let server = Server::start().await;
    let data = blob(MIB, 3);
    let item = spec(
        "tts",
        K::Tts,
        &server,
        &[("g.onnx", "/g.onnx", &data, true)],
        Install::Files,
    );
    let h = harness(vec![item], None, options(2));
    server.with(|s| s.stall_next_after = Some(300_000));
    h.app.download("tts").await.unwrap();
    let part = h.paths.local.join("downloads/tts/g.onnx.part");
    for _ in 0..500 {
        if std::fs::metadata(&part).is_ok_and(|m| m.len() >= 300_000) {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    let paused = h.app.cancel("tts").await.unwrap();
    assert_eq!(paused.state, S::Paused, "{paused:?}");
    assert_eq!(paused.progress.as_ref().map(|p| p.done), Some(300_000));
    h.app.download("tts").await.unwrap();
    h.wait("tts", S::Installed).await;
    let ranges: Vec<_> = server
        .requests()
        .into_iter()
        .filter_map(|(_, r)| r)
        .collect();
    assert_eq!(ranges, ["bytes=300000-"]);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn wrong_pinned_hash_deletes_the_file() {
    let server = Server::start().await;
    let data = blob(MIB, 4);
    let mut item = spec(
        "bad",
        K::Stt,
        &server,
        &[("b.bin", "/b.bin", &data, true)],
        Install::Files,
    );
    item.files[0].sha256 = Some("0".repeat(64));
    let h = harness(vec![item], None, options(2));
    h.app.download("bad").await.unwrap();
    let failed = h.wait("bad", S::Failed).await;
    assert!(failed.error.unwrap().contains("plik usunięty"));
    assert!(!h.paths.local.join("downloads/bad/b.bin").exists());
    assert!(!h.paths.local.join("downloads/bad/b.bin.part").exists());
    assert!(!h.paths.models().join("t/bad/b.bin").exists());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn unpinned_file_waits_for_trust_with_computed_hash() {
    let server = Server::start().await;
    let data = blob(MIB, 5);
    let item = spec(
        "tofu",
        K::Speaker,
        &server,
        &[("s.onnx", "/s.onnx", &data, false)],
        Install::Files,
    );
    let h = harness(vec![item], None, options(2));
    h.app.download("tofu").await.unwrap();
    let pending = h.wait("tofu", S::NeedsTrust).await;
    assert!(!pending.pinned);
    assert_eq!(
        pending.files[0].sha256.as_deref(),
        Some(sha(&data).as_str())
    );
    assert!(
        !h.paths.models().join("t/tofu/s.onnx").exists(),
        "bez zgody nic nie trafia na miejsce"
    );
    let wrong = TrustedHashes::from([("s.onnx".into(), "f".repeat(64))]);
    assert!(h.app.trust_hash("tofu", wrong).await.is_err());
    assert!(
        h.app
            .trust_hash("tofu", TrustedHashes::new())
            .await
            .is_err()
    );
    let right = TrustedHashes::from([("s.onnx".into(), sha(&data).to_uppercase())]);
    h.app.trust_hash("tofu", right).await.unwrap();
    h.wait("tofu", S::Installed).await;
    assert_eq!(
        std::fs::read(h.paths.models().join("t/tofu/s.onnx")).unwrap(),
        data
    );
    let receipt = std::fs::read_to_string(h.paths.state().join("models/tofu.json")).unwrap();
    assert!(receipt.contains("\"trusted\": true"), "{receipt}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn file_changed_after_download_is_rejected_on_trust() {
    let server = Server::start().await;
    let data = blob(MIB, 6);
    let item = spec(
        "swap",
        K::Tts,
        &server,
        &[("p.onnx", "/p.onnx", &data, false)],
        Install::Files,
    );
    let h = harness(vec![item], None, options(2));
    h.app.download("swap").await.unwrap();
    h.wait("swap", S::NeedsTrust).await;
    std::fs::write(h.paths.local.join("downloads/swap/p.onnx"), b"podmieniony").unwrap();
    let shown = TrustedHashes::from([("p.onnx".into(), sha(&data))]);
    h.app.trust_hash("swap", shown).await.unwrap();
    let failed = h.wait("swap", S::Failed).await;
    assert!(failed.error.unwrap().contains("zmienił się"));
    assert!(!h.paths.models().join("t/swap/p.onnx").exists());
    assert!(!h.paths.local.join("downloads/swap").exists());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn parallel_limit_queues_the_second_item() {
    let server = Server::start().await;
    let (a, b) = (blob(MIB, 7), blob(MIB, 8));
    let one = spec(
        "one",
        K::Stt,
        &server,
        &[("a.bin", "/a.bin", &a, true)],
        Install::Files,
    );
    let two = spec(
        "two",
        K::Stt,
        &server,
        &[("b.bin", "/b.bin", &b, true)],
        Install::Files,
    );
    let h = harness(vec![one, two], None, options(1));
    server.with(|s| s.stall_next_after = Some(1000));
    h.app.download("one").await.unwrap();
    h.wait("one", S::Downloading).await;
    h.app.download("two").await.unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    assert_eq!(h.item("two").await.state, S::Queued);
    h.app.cancel("one").await.unwrap();
    h.wait("two", S::Installed).await;
    assert_eq!(h.item("one").await.state, S::Paused);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn oversized_body_and_plain_http_are_rejected() {
    let server = Server::start().await;
    let data = blob(70 * MIB, 9);
    let mut big = spec(
        "big",
        K::Stt,
        &server,
        &[("g.bin", "/g.bin", &data, false)],
        Install::Files,
    );
    big.files[0].size = 1024;
    let plain = spec(
        "plain",
        K::Stt,
        &server,
        &[("p.bin", "/p.bin", b"x", true)],
        Install::Files,
    );
    let h = harness(vec![big], None, options(2));
    h.app.download("big").await.unwrap();
    assert!(
        h.wait("big", S::Failed)
            .await
            .error
            .unwrap()
            .contains("limit")
    );
    let mut opts = options(2);
    opts.loopback_http = false;
    let strict = common::harness(vec![plain], None, opts);
    strict.app.download("plain").await.unwrap();
    assert!(
        strict
            .wait("plain", S::Failed)
            .await
            .error
            .unwrap()
            .contains("https")
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn gguf_is_visible_to_providers_local_and_verify_detects_corruption() {
    let server = Server::start().await;
    let data = blob(MIB, 10);
    let item = spec(
        "llm",
        K::Llm,
        &server,
        &[("m.gguf", "/m.gguf", &data, true)],
        Install::Gguf,
    );
    let h = harness(vec![item], None, options(2));
    h.app.download("llm").await.unwrap();
    h.wait("llm", S::Installed).await;
    let model = h.paths.models().join("t/llm/m.gguf");
    let record = std::fs::read_to_string(providers_local_impl::hash_path(&model)).unwrap();
    assert_eq!(record.trim(), sha(&data));
    assert_eq!(h.app.verify("llm").await.unwrap().state, S::Installed);
    std::fs::write(&model, b"uszkodzony").unwrap();
    let corrupt = h.app.verify("llm").await.unwrap();
    assert_eq!(corrupt.state, S::Corrupt, "{corrupt:?}");
    let removed = h.app.remove("llm").await.unwrap();
    assert_eq!(removed.state, S::Missing);
    assert!(!model.exists() && !providers_local_impl::hash_path(&model).exists());
    assert!(!h.paths.state().join("models/llm.json").exists());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn sidecar_archive_installs_tree_without_prefix() {
    let server = Server::start().await;
    let zip = common_zip(&[
        ("Release/srv", b"exe" as &[u8]),
        ("Release/lib/x.dll", b"dll"),
        ("README.md", b"r"),
    ]);
    let item = spec(
        "srv",
        K::Sidecar,
        &server,
        &[("srv.zip", "/srv.zip", &zip, true)],
        Install::Tree {
            strip: "Release/".into(),
            require: vec!["srv".into()],
        },
    );
    let h = harness(vec![item], None, options(2));
    h.app.download("srv").await.unwrap();
    h.wait("srv", S::Installed).await;
    let dir = h.paths.models().join("t/srv");
    assert_eq!(std::fs::read(dir.join("srv")).unwrap(), b"exe");
    assert_eq!(std::fs::read(dir.join("lib/x.dll")).unwrap(), b"dll");
    assert!(!dir.join("README.md").exists() && !dir.join("Release").exists());
    std::fs::write(dir.join("lib/x.dll"), b"podmiana").unwrap();
    assert_eq!(h.app.verify("srv").await.unwrap().state, S::Corrupt);
}

fn common_zip(entries: &[(&str, &[u8])]) -> Vec<u8> {
    use std::io::Write;
    let mut out = std::io::Cursor::new(Vec::new());
    let mut zip = zip::ZipWriter::new(&mut out);
    for (name, bytes) in entries {
        zip.start_file(*name, zip::write::SimpleFileOptions::default())
            .unwrap();
        zip.write_all(bytes).unwrap();
    }
    zip.finish().unwrap();
    out.into_inner()
}
