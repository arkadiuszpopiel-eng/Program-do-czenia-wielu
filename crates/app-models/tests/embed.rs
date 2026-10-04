//! Embedder w ścieżce `app-*` na zabawkowym modelu ONNX z `lib-embed` (`testkit`): pobranie
//! z serwera → `embed.json` z SHA-256 → aktywacja (zapis ustawienia, `preload`) → przebudowa
//! wektorów baz sesji i baz zakresów pamięci w tle (zdarzenia `ReindexStatus`) → wyszukiwanie
//! wektorowe nowym modelem; powrót do embeddera leksykalnego; ochrona aktywnego modelu;
//! wybór przy starcie (`startup_embedder`).

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::sync::Arc;
use std::time::Duration;

use app_api::dto::{AlfaEvent, ModelItemKind as K, ModelItemState as S};
use app_models::catalog::Install;
use app_models::{EMBEDDER_KEY, EmbedDeps, LEXICAL, startup_embedder};
use common::http::Server;
use common::{harness, options, spec};
use core_config_contract::{ConfigKey, ConfigStore, MachineId, Scope};
use core_config_fake::FakeConfigStore;
use lib_embed::testkit::{TOY_DIMS, TOY_TOKENIZER_JSON, TOY_VOCAB, toy_manifest, toy_model_bytes};
use memory_contract::MemoryScope;
use memory_impl::{ScopeDbs, VaultScopeDbs};
use search_contract::{
    Caller, Doc, DocId, DocKind, Mode, Query, Search, SessionId, TxIndexer, VectorStatus,
};
use search_fake::HashEmbedder;
use search_impl::SqliteSearch;
use sessions_fake::{MemoryKeyVault, TempDbProvider};

const TEXTS: [&str; 4] = [
    "Karolina lubi zielony kolor i żaglówki",
    "Spotkanie zespołu przeniesione na czwartek",
    "Faktura za prąd do zapłaty w piątek",
    "Przepis na pierogi z kapustą i grzybami",
];

fn doc(session: &str, kind: DocKind, key: &str, text: &str) -> Doc {
    Doc {
        id: DocId::new(kind, key),
        session: SessionId::new(session),
        text: text.into(),
        ts: Default::default(),
    }
}

async fn wait_ready(search: &SqliteSearch, session: &SessionId, embedder: &str) {
    for _ in 0..1000 {
        if let Ok(VectorStatus::Ready {
            embedder: e,
            missing: 0,
        }) = search.vector_status(session)
            && e.starts_with(embedder)
        {
            return;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    panic!(
        "{session}: wektory nie gotowe: {:?}",
        search.vector_status(session)
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn activation_rebuilds_vectors_and_vector_search_uses_the_model() {
    let server = Server::start().await;
    let model = toy_model_bytes(TOY_VOCAB, TOY_DIMS, false);
    let mut template = toy_manifest(&model, TOY_TOKENIZER_JSON.as_bytes());
    template.model.sha256.clear();
    template.tokenizer.sha256.clear();
    // Bez prefiksów E5: zapytanie o dokładny tekst dokumentu = ten sam wektor.
    template.query_prefix.clear();
    template.passage_prefix.clear();
    let mut item = spec(
        "toy-encoder",
        K::Embed,
        &server,
        &[
            ("model.onnx", "/toy/model.onnx", &model, true),
            (
                "tokenizer.json",
                "/toy/tokenizer.json",
                TOY_TOKENIZER_JSON.as_bytes(),
                true,
            ),
        ],
        Install::Embed(Box::new(template)),
    );
    item.dir = "embed/toy-encoder".into();
    // Bazy: sesja „A” (dostawca sesji) i zakres globalny pamięci (baza własna z kluczem w sejfie).
    let provider = Arc::new(TempDbProvider::new().unwrap());
    let lexical = Arc::new(HashEmbedder::new());
    let search = Arc::new(SqliteSearch::new(provider.clone(), lexical.clone()).unwrap());
    for (i, text) in TEXTS.iter().enumerate() {
        search
            .index(&doc("A", DocKind::Turn, &i.to_string(), text))
            .unwrap();
    }
    let dir = tempfile::tempdir().unwrap();
    let scopes: Arc<dyn ScopeDbs> = Arc::new(VaultScopeDbs::new(
        dir.path().join("memory"),
        Arc::new(MemoryKeyVault::new()),
        provider.clone(),
    ));
    let global = scopes.db(&MemoryScope::Global, true).unwrap().unwrap();
    global
        .with(|c| {
            search.prepare(c)?;
            search.index_in(
                c,
                &doc("@global", DocKind::Memory, "f1", "Ulubiony kolor: zielony"),
            )
        })
        .unwrap();
    let config = Arc::new(FakeConfigStore::new(MachineId::new("t")));
    let deps = EmbedDeps {
        search: search.clone(),
        lexical,
        residency: None,
        scopes: Some(scopes),
        config: config.clone(),
    };
    let h = harness(vec![item], Some(deps), options(2));
    h.app.download("toy-encoder").await.unwrap();
    h.wait("toy-encoder", S::Installed).await;
    let manifest = h.paths.models().join("embed/toy-encoder/embed.json");
    assert!(lib_embed::EmbedManifest::load(&manifest).is_ok());
    // Model nie jest jeszcze wybrany — wyszukiwanie zostaje leksykalne.
    assert!(!h.item("toy-encoder").await.active);
    let view = h.app.activate_embedder("toy-encoder").await.unwrap();
    assert_eq!(
        (view.configured.as_str(), view.active.as_str()),
        ("toy-encoder", "toy-encoder")
    );
    assert!(
        view.index_id.starts_with("toy-encoder@") && view.dims == 32,
        "{view:?}"
    );
    let key = ConfigKey::new(EMBEDDER_KEY).unwrap();
    let saved = config.get(&key, &Scope::Global).await.unwrap();
    assert_eq!(saved, Some(serde_json::json!("toy-encoder")));
    let a = SessionId::new("A");
    for _ in 0..1000 {
        if h.app.reindex_status().await.unwrap().finished {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    wait_ready(&search, &a, "toy-encoder@").await;
    global
        .with(|c| {
            assert!(search.vector_status_in(c)?.vectors_usable());
            Ok::<(), search_contract::SearchError>(())
        })
        .unwrap();
    let status = h.app.reindex_status().await.unwrap();
    assert!(
        status.finished && !status.running && status.failed == 0,
        "{status:?}"
    );
    assert!(status.embedded >= 5, "{status:?}");
    assert!(h.item("toy-encoder").await.active);
    // Pełny przebieg zapisany — start aplikacji nie otworzy wszystkich baz ponownie (do tygodnia).
    let marker = h.paths.state().join("models").join("reindex.json");
    assert!(app_models::embed::fresh_pass(&marker, &view.index_id));
    assert!(!app_models::embed::fresh_pass(&marker, "inny/32"));
    // Wyszukiwanie wektorowe nowym modelem: dokładny tekst dokumentu → on sam na górze.
    for (i, text) in TEXTS.iter().enumerate() {
        let query = Query {
            mode: Mode::Vector,
            ..Query::in_session(a.clone(), *text, 3)
        };
        let hits = search.query(&query, &Caller::Owner).unwrap();
        assert_eq!(
            hits.first().map(|h| h.doc.key.clone()),
            Some(i.to_string()),
            "{text}"
        );
    }
    let removed = h.app.remove("toy-encoder").await;
    assert!(removed.is_err(), "aktywnego modelu nie wolno usunąć");
    assert!(
        h.events()
            .iter()
            .any(|e| matches!(e, AlfaEvent::ReindexStatus { status } if status.finished))
    );
    // Powrót do leksykalnego: przebudowa do embeddera leksykalnego, model można usunąć.
    let back = h.app.activate_embedder(LEXICAL).await.unwrap();
    assert_eq!(back.active, LEXICAL);
    wait_ready(&search, &a, "fake-hash").await;
    assert_eq!(h.app.remove("toy-encoder").await.unwrap().state, S::Missing);
    assert!(
        h.app.activate_embedder("toy-encoder").await.is_err(),
        "niezainstalowany"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn startup_choice_follows_setting_and_installation() {
    let dir = tempfile::tempdir().unwrap();
    let models = dir.path().join("models");
    let lexical: Arc<dyn search_contract::Embedder> = Arc::new(HashEmbedder::new());
    let lex_id = lexical.model_id().to_owned();
    let pick = |v: Option<serde_json::Value>| {
        startup_embedder(&models, v.as_ref(), lexical.clone())
            .model_id()
            .to_owned()
    };
    assert_eq!(pick(None), lex_id, "model domyślny niezainstalowany");
    lib_embed::testkit::write_toy_model(&models.join("embed").join("toy")).unwrap();
    assert!(pick(Some(serde_json::json!("toy"))).starts_with("toy-encoder@"));
    assert_eq!(pick(Some(serde_json::json!(LEXICAL))), lex_id);
    assert_eq!(pick(Some(serde_json::json!("brak"))), lex_id);
}
