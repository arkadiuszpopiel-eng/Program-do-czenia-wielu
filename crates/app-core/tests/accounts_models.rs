//! Wybór modelu konta API i kolejki modeli (zgłoszenie właściciela, 2026-10-09: klucz OpenAI
//! działał, lista miała 133 modele, a każda wiadomość kończyła się błędem „The model `tts-1` does
//! not exist” — Router brał pierwszy model z listy; do tego nie dało się wybrać modeli). Cała
//! ścieżka idzie przez prawdziwy adapter i Router do lokalnego serwera udającego dostawcę.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use app_core::dto::SessionTemplate;
use common::chat_stub::{MISSING, chat_stub};
use common::keys::{core, input, listed};
use common::{ends, send, until};

const KEY: &str = "test-modele-XXXXXXXXXXXXXXXXXXXX";

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn chat_uses_a_chat_model_then_the_queue_and_skips_an_unavailable_model() {
    let (base, asked) = chat_stub();
    let (core, _dir) = core().await;
    let mut rx = core.subscribe_events();
    let account = core.accounts_add(input(KEY, &base, "praca")).await.unwrap();
    assert!(core.accounts_test(account.id.clone()).await.unwrap().ok);

    // Lista jest opisana: wiadomo, który model służy do czego, i który Alfa wybierze sama.
    let seen = listed(&core, &account.id).await;
    let purpose = |id: &str| {
        seen.models
            .iter()
            .find(|m| m.id == id)
            .map(|m| m.purpose.clone())
            .unwrap_or_default()
    };
    assert_eq!(purpose("tts-1"), "speech");
    assert_eq!(purpose("whisper-1"), "transcription");
    assert_eq!(purpose("text-embedding-3-small"), "embedding");
    assert_eq!(purpose("gpt-4o"), "chat");
    assert_eq!(seen.auto_model.as_deref(), Some("gpt-4o"));
    assert!(
        seen.models
            .iter()
            .any(|m| m.snapshot && m.id == "gpt-4o-2024-08-06")
    );
    assert!(seen.queues.is_empty());

    let session = core
        .sessions_create(SessionTemplate::Empty)
        .await
        .unwrap()
        .id;
    let ask = |text: &'static str| {
        let (core, session) = (core.clone(), session.clone());
        async move {
            core.turns_send(session, send(text, None))
                .await
                .unwrap()
                .assistant_turn_id
                .unwrap()
        }
    };
    let answer = |turn: String| {
        let (core, session) = (core.clone(), session.clone());
        async move {
            let snap = core.turns_list(session).await.unwrap();
            snap.turns
                .iter()
                .find(|t| t.id == turn)
                .map(|t| t.text.clone())
                .unwrap_or_default()
        }
    };

    // 1. Bez kolejki: rozmowa idzie do modelu rozmowy, nie do pierwszego z listy (`tts-1`).
    let turn = ask("Hej!").await;
    until(&mut rx, ends(&turn)).await;
    assert_eq!(*asked.lock().unwrap(), ["gpt-4o"]);
    assert_eq!(answer(turn).await, "Odpowiada gpt-4o.");

    // 2. Kolejka dla rozmowy: pierwszy model z kolejki.
    let queued = core
        .accounts_set_queue(
            account.id.clone(),
            "chat".into(),
            vec!["gpt-4o-mini".into(), "gpt-4o".into()],
        )
        .await
        .unwrap();
    assert_eq!(queued.queues["chat"], ["gpt-4o-mini", "gpt-4o"]);
    asked.lock().unwrap().clear();
    let turn = ask("Drugie pytanie").await;
    until(&mut rx, ends(&turn)).await;
    assert_eq!(*asked.lock().unwrap(), ["gpt-4o-mini"]);

    // 3. Model z kolejki, którego konto nie ma: Router przechodzi do następnego i odpowiada.
    core.accounts_set_queue(
        account.id.clone(),
        "chat".into(),
        vec![MISSING.into(), "gpt-4o-mini".into()],
    )
    .await
    .unwrap();
    asked.lock().unwrap().clear();
    let turn = ask("Trzecie pytanie").await;
    until(&mut rx, ends(&turn)).await;
    assert_eq!(*asked.lock().unwrap(), [MISSING, "gpt-4o-mini"]);
    assert_eq!(answer(turn).await, "Odpowiada gpt-4o-mini.");

    // 4. Pusta kolejka przywraca wybór automatyczny.
    let cleared = core
        .accounts_set_queue(account.id.clone(), "chat".into(), Vec::new())
        .await
        .unwrap();
    assert!(cleared.queues.is_empty());
    asked.lock().unwrap().clear();
    let turn = ask("Czwarte pytanie").await;
    until(&mut rx, ends(&turn)).await;
    assert_eq!(*asked.lock().unwrap(), ["gpt-4o"]);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn queue_rules_and_suggestions() {
    let (base, _asked) = chat_stub();
    let (core, _dir) = core().await;
    let account = core.accounts_add(input(KEY, &base, "praca")).await.unwrap();
    assert!(core.accounts_test(account.id.clone()).await.unwrap().ok);
    let set = |class: &'static str, models: Vec<&'static str>| {
        let (core, id) = (core.clone(), account.id.clone());
        async move {
            core.accounts_set_queue(
                id,
                class.into(),
                models.into_iter().map(String::from).collect(),
            )
            .await
        }
    };

    // Model syntezy mowy nie wejdzie do kolejki rozmowy — z nazwanym powodem.
    let speech = set("chat", vec!["gpt-4o", "tts-1"]).await.unwrap_err();
    assert!(
        speech.message.contains("syntezy mowy"),
        "{}",
        speech.message
    );
    let twice = set("code", vec!["gpt-4o", "gpt-4o"]).await.unwrap_err();
    assert!(twice.message.contains("dwa razy"), "{}", twice.message);
    let many: Vec<&'static str> = vec![
        "m-1", "m-2", "m-3", "m-4", "m-5", "m-6", "m-7", "m-8", "m-9", "m-10", "m-11",
    ];
    assert!(set("chat", many).await.is_err());
    assert!(set("zmyślona", vec!["gpt-4o"]).await.is_err());
    assert!(listed(&core, &account.id).await.queues.is_empty());

    // Embeddingi mają własną kolejkę i własne modele.
    let ok = set("embeddings", vec!["text-embedding-3-small"])
        .await
        .unwrap();
    assert_eq!(ok.queues["embeddings"], ["text-embedding-3-small"]);
    assert!(set("embeddings", vec!["gpt-4o"]).await.is_err());

    let suggest = |class: &'static str, strategy: &'static str| {
        let (core, id) = (core.clone(), account.id.clone());
        async move {
            core.accounts_suggest_queue(id, class.into(), strategy.into())
                .await
                .unwrap()
        }
    };
    assert_eq!(
        suggest("chat", "quality").await,
        ["gpt-4o", "gpt-4o-2024-08-06", "gpt-4o-mini"]
    );
    assert_eq!(suggest("chat", "economy").await[0], "gpt-4o-mini");
    assert_eq!(
        suggest("embeddings", "quality").await,
        ["text-embedding-3-small"]
    );
    assert!(
        core.accounts_suggest_queue(account.id.clone(), "chat".into(), "losowa".into())
            .await
            .is_err()
    );
    // Propozycja niczego nie zapisuje.
    assert!(!listed(&core, &account.id).await.queues.contains_key("chat"));
}
