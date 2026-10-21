//! Przypięte sumy katalogu (C5.1, przegląd 2026-10-21): stan modelu GGUF zgodny z kryterium
//! `providers-local` (rekord równy przypiętej sumie), plik towarzyszący (tekst licencji) nie decyduje
//! o stanie pozycji, a zgoda TOFU sprzed przypięcia nie instaluje pliku innego niż przypięty.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use app_api::dto::{ModelItemKind as K, ModelItemState as S, TrustedHashes};
use app_models::catalog::Install;
use common::http::Server;
use common::{Harness, blob, harness, harness_in, options, sha, spec};

const MIB: usize = 1024 * 1024;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn gguf_record_must_match_the_pinned_hash() {
    let server = Server::start().await;
    let data = blob(MIB, 21);
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
    let record = providers_local_impl::hash_path(&model);
    // Rekord z innym hashem: providers-local uzna model za niepobrany — menedżer też nie „Zainstalowano”.
    std::fs::write(&record, format!("{}\n", "e".repeat(64))).unwrap();
    assert_eq!(h.item("llm").await.state, S::Corrupt);
    // Bez potwierdzenia instalacji (np. kopia katalogu): plik obecny, rekord niezgodny — „zewnętrzny”.
    std::fs::remove_file(h.paths.state().join("models/llm.json")).unwrap();
    assert_eq!(h.item("llm").await.state, S::External);
    std::fs::write(&record, format!("{}\n", sha(&data))).unwrap();
    assert_eq!(h.item("llm").await.state, S::Installed);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn companion_license_does_not_decide_the_state() {
    let server = Server::start().await;
    let data = blob(MIB, 22);
    let license = b"Open RAIL-M".to_vec();
    let item = spec(
        "tts",
        K::Tts,
        &server,
        &[
            ("v.onnx", "/v.onnx", &data, true),
            ("LICENSE", "/LICENSE", &license, true),
        ],
        Install::Files,
    );
    let h = harness(vec![item], None, options(2));
    // Kopia sprzed dodania licencji (bez potwierdzenia instalacji): pozycja „zewnętrzna”, nie brakująca.
    let target = h.paths.models().join("t/tts");
    std::fs::create_dir_all(&target).unwrap();
    std::fs::write(target.join("v.onnx"), &data).unwrap();
    assert_eq!(h.item("tts").await.state, S::External);
    // Nowa instalacja pobiera też tekst licencji.
    std::fs::remove_dir_all(&target).unwrap();
    h.app.download("tts").await.unwrap();
    h.wait("tts", S::Installed).await;
    assert_eq!(std::fs::read(target.join("LICENSE")).unwrap(), license);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn trust_from_before_pinning_cannot_install_another_file() {
    let server = Server::start().await;
    let data = blob(MIB, 23);
    let unpinned = spec(
        "tofu",
        K::Speaker,
        &server,
        &[("s.onnx", "/s.onnx", &data, false)],
        Install::Files,
    );
    let h = harness(vec![unpinned.clone()], None, options(2));
    h.app.download("tofu").await.unwrap();
    h.wait("tofu", S::NeedsTrust).await;
    // Aktualizacja katalogu: ta sama pozycja z przypiętą sumą innego pliku (pobrany ≠ przypięty).
    let Harness { dir, .. } = h;
    let mut pinned = unpinned;
    pinned.files[0].sha256 = Some(sha(&blob(MIB, 24)));
    let h = harness_in(dir, vec![pinned], None, options(2));
    let shown = TrustedHashes::from([("s.onnx".into(), sha(&data))]);
    h.app.trust_hash("tofu", shown).await.unwrap();
    let failed = h.wait("tofu", S::Failed).await;
    assert!(failed.error.unwrap_or_default().contains("zmienił się"));
    assert!(!h.paths.models().join("t/tofu/s.onnx").exists());
}
