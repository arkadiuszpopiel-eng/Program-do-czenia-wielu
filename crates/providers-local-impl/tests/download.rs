//! Pobieranie modeli (ACC-F1-providers-local-02): przerwanie połączenia → wznowienie HTTP Range,
//! hash zgodny; zły hash → błąd i usunięcie `.part`; zaufanie przy pierwszym pobraniu; anulowanie;
//! walidacja manifestu (bez kwantów IQ); zdarzenia postępu. Lokalny serwer HTTP, bez internetu.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod support;

use std::sync::{Arc, Mutex};

use core_bus_fake::FakeBus;
use core_registry_contract::{Module, ModuleContext};
use providers_contract::CancellationToken;
use providers_local_impl::{
    Downloader, LocalError, LocalModule, MAX_RESUMES, builtin_models, hash_path, installed,
    is_iq_quant, parse_manifest, part_path,
};
use sha2::{Digest, Sha256};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

#[derive(Default)]
struct ServerState {
    ranges: Vec<Option<String>>,
    /// Ile kolejnych odpowiedzi ma zostać urwanych w połowie.
    cut: u32,
}

struct FileServer {
    url: String,
    state: Arc<Mutex<ServerState>>,
}

async fn serve(data: Arc<Vec<u8>>, cut: u32) -> FileServer {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}/model.gguf", listener.local_addr().unwrap());
    let state = Arc::new(Mutex::new(ServerState {
        cut,
        ..ServerState::default()
    }));
    let st = state.clone();
    tokio::spawn(async move {
        while let Ok((mut sock, _)) = listener.accept().await {
            let data = data.clone();
            let st = st.clone();
            tokio::spawn(async move {
                let mut buf = vec![0u8; 4096];
                let n = sock.read(&mut buf).await.unwrap_or(0);
                let head = String::from_utf8_lossy(&buf[..n]).to_string();
                let range = head
                    .lines()
                    .find(|l| l.to_ascii_lowercase().starts_with("range:"))
                    .map(|l| l[6..].trim().to_owned());
                let cut = {
                    let mut s = st.lock().unwrap();
                    s.ranges.push(range.clone());
                    let c = s.cut > 0;
                    s.cut = s.cut.saturating_sub(1);
                    c
                };
                let start: usize = range
                    .as_deref()
                    .and_then(|r| r.strip_prefix("bytes="))
                    .and_then(|r| r.trim_end_matches('-').parse().ok())
                    .unwrap_or(0);
                let total = data.len();
                let head = if start > 0 {
                    format!(
                        "HTTP/1.1 206 Partial Content\r\ncontent-length: {}\r\ncontent-range: bytes {start}-{}/{total}\r\nconnection: close\r\n\r\n",
                        total - start,
                        total - 1
                    )
                } else {
                    format!(
                        "HTTP/1.1 200 OK\r\ncontent-length: {total}\r\nconnection: close\r\n\r\n"
                    )
                };
                let _ = sock.write_all(head.as_bytes()).await;
                let body = &data[start..];
                let body = if cut { &body[..body.len() / 2] } else { body };
                let _ = sock.write_all(body).await;
                let _ = sock.shutdown().await;
            });
        }
    });
    FileServer { url, state }
}

fn payload() -> Arc<Vec<u8>> {
    Arc::new((0..300_000u32).map(|i| (i % 251) as u8).collect())
}

fn sha(data: &[u8]) -> String {
    Sha256::digest(data)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

#[tokio::test]
async fn interrupted_download_resumes_with_range_and_verifies_hash() {
    let data = payload();
    let server = serve(data.clone(), 2).await;
    let dir = tempfile::tempdir().unwrap();
    let mut entry = support::entry(&server.url);
    entry.sha256 = sha(&data);
    let progress = Mutex::new(Vec::new());
    let out = Downloader::new(dir.path())
        .unwrap()
        .download(&entry, &CancellationToken::new(), &|p| {
            progress.lock().unwrap().push(p)
        })
        .await
        .unwrap();
    assert!(out.verified);
    assert_eq!(out.sha256, sha(&data));
    assert_eq!(std::fs::read(&out.path).unwrap(), *data);
    assert!(!part_path(&out.path).exists());
    assert!(installed(dir.path(), &entry));
    let ranges = server.state.lock().unwrap().ranges.clone();
    assert_eq!(ranges.len(), 3, "2 przerwania + dokończenie");
    assert_eq!(ranges[0], None);
    assert_eq!(ranges[1].as_deref(), Some("bytes=150000-"));
    assert!(
        ranges[2]
            .as_deref()
            .is_some_and(|r| r.starts_with("bytes=2"))
    );
    let p = progress.lock().unwrap().clone();
    assert!(p.iter().any(|p| p.resumed));
    assert_eq!(
        p.last().map(|p| (p.bytes, p.total)),
        Some((300_000, Some(300_000)))
    );
    // Drugie wywołanie: już zainstalowany, bez ruchu sieciowego.
    Downloader::new(dir.path())
        .unwrap()
        .download(&entry, &CancellationToken::new(), &|_| {})
        .await
        .unwrap();
    assert_eq!(server.state.lock().unwrap().ranges.len(), 3);
}

#[tokio::test]
async fn bad_hash_fails_and_removes_partial_file() {
    let data = payload();
    let server = serve(data, 0).await;
    let dir = tempfile::tempdir().unwrap();
    let mut entry = support::entry(&server.url);
    entry.sha256 = "0".repeat(64);
    let err = Downloader::new(dir.path())
        .unwrap()
        .download(&entry, &CancellationToken::new(), &|_| {})
        .await
        .unwrap_err();
    assert!(matches!(err, LocalError::HashMismatch { .. }), "{err}");
    let path = dir.path().join(&entry.file);
    assert!(!path.exists() && !part_path(&path).exists());
    assert!(!installed(dir.path(), &entry));
}

#[tokio::test]
async fn unknown_hash_is_recorded_on_first_download_and_too_many_cuts_fail() {
    let data = payload();
    let server = serve(data.clone(), 0).await;
    let dir = tempfile::tempdir().unwrap();
    let entry = support::entry(&server.url);
    let out = Downloader::new(dir.path())
        .unwrap()
        .download(&entry, &CancellationToken::new(), &|_| {})
        .await
        .unwrap();
    assert!(
        !out.verified,
        "manifest bez hasha: zaufanie przy pierwszym użyciu"
    );
    let recorded = std::fs::read_to_string(hash_path(&out.path)).unwrap();
    assert_eq!(recorded.trim(), sha(&data));
    // Więcej przerwań niż MAX_RESUMES → błąd, a `.part` zostaje do późniejszego wznowienia.
    let flaky = serve(data, MAX_RESUMES + 5).await;
    let dir2 = tempfile::tempdir().unwrap();
    let e2 = support::entry(&flaky.url);
    let err = Downloader::new(dir2.path())
        .unwrap()
        .download(&e2, &CancellationToken::new(), &|_| {})
        .await
        .unwrap_err();
    assert!(matches!(err, LocalError::Download(_)), "{err}");
    assert!(part_path(&dir2.path().join(&e2.file)).exists());
    // Anulowanie przed startem.
    let cancel = CancellationToken::new();
    cancel.cancel();
    let err = Downloader::new(dir2.path())
        .unwrap()
        .download(&e2, &cancel, &|_| {})
        .await
        .unwrap_err();
    assert_eq!(err, LocalError::Cancelled);
}

#[tokio::test]
async fn module_download_publishes_progress_events() {
    let data = payload();
    let server = serve(data, 1).await;
    let env = support::Env::new();
    let mut entry = support::entry(&server.url);
    entry.file = "inny.Q4_K_M.gguf".into();
    let sidecar = providers_local_impl::Sidecar::new(
        env.config(),
        Arc::new(env.launcher("ok", None)),
        None,
        None,
    )
    .unwrap();
    let provider = Arc::new(providers_local_impl::LocalProvider::new(
        vec![entry],
        sidecar,
    ));
    let mut module = LocalModule::new(provider, std::time::Duration::from_secs(3600)).unwrap();
    let bus = FakeBus::default();
    module
        .start(ModuleContext::new(
            module.manifest().id.clone(),
            Arc::new(bus.clone()),
        ))
        .await
        .unwrap();
    module
        .download(support::MODEL, &CancellationToken::new())
        .await
        .unwrap();
    assert!(
        module
            .download("brak", &CancellationToken::new())
            .await
            .is_err()
    );
    for _ in 0..200 {
        if bus
            .recorded()
            .iter()
            .any(|e| e.kind.as_str() == "local.model.download.finished")
        {
            break;
        }
        tokio::task::yield_now().await;
    }
    let names: Vec<String> = bus
        .recorded()
        .iter()
        .map(|e| e.kind.as_str().to_owned())
        .collect();
    assert!(names.contains(&"local.model.download.progress".to_owned()));
    assert!(names.contains(&"local.model.download.finished".to_owned()));
    module.stop().await.unwrap();
}

#[test]
fn manifest_validation_rejects_iq_quants_and_bad_entries() {
    let models = builtin_models().unwrap();
    assert_eq!(models.len(), 2);
    let bielik = &models[0];
    assert!(bielik.id.starts_with("bielik-4.5b"));
    assert_eq!(bielik.quant, "Q8_0");
    assert!(bielik.params_b >= 3.0 && bielik.params_b <= 4.8);
    assert_eq!(models[1].quant, "Q8_0");
    assert!(models[1].params_b < 2.0 && models[1].size_mb < bielik.size_mb);
    for m in &models {
        // Tylko oficjalne repozytorium autorów (speakleash), plik z nazwy wpisu.
        let repo = "https://huggingface.co/speakleash/Bielik-";
        assert!(m.url.starts_with(repo), "{}", m.url);
        assert!(
            m.url.ends_with(&format!("/resolve/main/{}", m.file)),
            "{}",
            m.url
        );
        assert!(m.id.ends_with("-q8_0") && m.file.ends_with(".Q8_0.gguf"));
    }
    for q in ["IQ4_XS", "iq3_m", "model.IQ2_XXS.gguf"] {
        assert!(is_iq_quant(q), "{q}");
    }
    for q in ["Q4_K_M", "Q5_0", "bielik-4.5b.Q4_K_M.gguf", "IQ"] {
        assert!(!is_iq_quant(q), "{q}");
    }
    let base = |extra: &str| {
        format!(
            "schema_version = 1\n[[model]]\nid = \"m\"\nname = \"M\"\nurl = \"https://x/m.gguf\"\nfile = \"m.Q4_K_M.gguf\"\nsize_mb = 1\nparams_b = 4.0\nquant = \"Q4_K_M\"\nlayers = 32\nctx = 4096\nvram_mb = 1\nram_mb = 1\nkv_mb_per_1k_ctx = 1\nlicense = \"x\"\n{extra}"
        )
    };
    assert!(parse_manifest(&base("")).is_ok());
    let bad = [
        base("").replace("Q4_K_M\"\nlayers", "IQ4_XS\"\nlayers"),
        base("").replace("m.Q4_K_M.gguf", "m.IQ3_M.gguf"),
        base("").replace("m.Q4_K_M.gguf", "../m.gguf"),
        base("").replace("m.Q4_K_M.gguf", "m.bin"),
        base("").replace("https://x", "http://x"),
        base("sha256 = \"abc\""),
        base("").replace("schema_version = 1", "schema_version = 2"),
        format!(
            "{}\n{}",
            base(""),
            base("").replace("schema_version = 1\n", "")
        ),
        base("").replace("layers = 32", "layers = 0"),
        base("").replace("kv_mb_per_1k_ctx = 1", "kv_mb_per_1k_ctx = 0"),
        base("").replace("kv_mb_per_1k_ctx = 1\n", ""),
        base("nieznane = 1"),
    ];
    for (i, text) in bad.iter().enumerate() {
        assert!(parse_manifest(text).is_err(), "przypadek {i}: {text}");
    }
}
