//! Rozmowa przez Router (`router-impl`) złożony w `AppCore`: fallback po 5xx na drugiego
//! kandydata bez utraty wiadomości (UI dostaje jedną odpowiedź), oś czasu i kapsuła aktywności
//! z decyzją, sesja prywatna tylko na trasach dozwolonych, profil lokalny przy kluczach API.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::sync::Arc;

use app_core::dto::{AlfaEvent, ModelProfile, SendOptions, SessionTemplate, TurnErrorCode};
use app_core::ports::HeadlessShell;
use app_core::{AppCore, AppPaths, NO_LOCAL, RouteKind};
use common::*;
use providers_contract::{ModelProvider, ProviderPrivacy};
use providers_fake::{FAKE_MODEL, FakeProvider, Script};
use sessions_contract::PrivacyTag;

fn fake(id: &str, answer: &str) -> FakeProvider {
    let words: Vec<String> = answer.split_inclusive(' ').map(str::to_owned).collect();
    FakeProvider::new(id).with_default_script(Script::chunks(
        FAKE_MODEL,
        &words,
        std::time::Duration::from_millis(1),
    ))
}

struct Routed {
    core: AppCore,
    rx: tokio::sync::broadcast::Receiver<app_core::EventBatch>,
    _dir: tempfile::TempDir,
}

async fn routed(providers: Vec<FakeProvider>) -> Routed {
    let dir = tempfile::tempdir().unwrap();
    let mut opts = options(None, Arc::new(HeadlessShell::default()));
    opts.providers = providers
        .into_iter()
        .map(|p| (Arc::new(p) as Arc<dyn ModelProvider>, RouteKind::Api))
        .collect();
    let core = AppCore::build(AppPaths::under(dir.path()), opts)
        .await
        .unwrap();
    let rx = core.subscribe_events();
    Routed {
        core,
        rx,
        _dir: dir,
    }
}

fn texts(events: &[AlfaEvent], turn: &str) -> String {
    events
        .iter()
        .filter_map(|e| match e {
            AlfaEvent::TextDelta { turn_id, text, .. } if turn_id == turn => Some(text.as_str()),
            _ => None,
        })
        .collect()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn router_falls_back_after_5xx_and_ui_gets_one_answer() {
    let anthropic = fake("anthropic", "Nie powinno się pokazać.");
    anthropic.push_script(Script::http_error(503, None));
    let openai = fake("openai", "Odpowiedź z zapasowego dostawcy.");
    let mut r = routed(vec![anthropic.clone(), openai.clone()]).await;
    let core = &r.core;
    assert!(core.system_status().await.unwrap().keys_configured);
    let sid = core
        .sessions_create(SessionTemplate::Empty)
        .await
        .unwrap()
        .id;
    let sent = core
        .turns_send(sid.clone(), send("Cześć", None))
        .await
        .unwrap();
    let turn = sent.assistant_turn_id.unwrap();
    let events = until(&mut r.rx, ends(&turn)).await;

    assert_eq!(texts(&events, &turn), "Odpowiedź z zapasowego dostawcy.");
    assert!(stop_reason(&events, &turn).is_some());
    assert_eq!(anthropic.calls().len(), 1);
    assert_eq!(openai.calls().len(), 1);
    // Ta sama wiadomość trafiła do zapasowego dostawcy (0 utraconych).
    assert_eq!(last_user_text(&openai.calls()[0]), "Cześć");
    let appended = events
        .iter()
        .filter(|e| matches!(e, AlfaEvent::TurnAppended { turn: t, .. } if t.id == turn))
        .count();
    assert_eq!(appended, 1, "UI dostaje jedną turę agentki");
    let decision = events
        .iter()
        .find_map(|e| match e {
            AlfaEvent::TimelineAppended { event } if event.title.starts_with("Router") => {
                Some(event.clone())
            }
            _ => None,
        })
        .expect("decyzja Routera na osi czasu");
    assert!(decision.title.contains("fake-model"), "{}", decision.title);
    let detail = decision.detail.unwrap_or_default();
    assert!(
        detail.contains("fallback z anthropic:fake-model"),
        "{detail}"
    );
    assert!(events.iter().any(|e| matches!(
        e,
        AlfaEvent::ActivityChanged { activity: Some(a), .. } if a.description.starts_with("Odpowiada")
    )));
    // Historia (append-only) ma jedną odpowiedź — od dostawcy, który odpowiedział.
    let snap = core.turns_list(sid).await.unwrap();
    let answer = snap.turns.iter().find(|t| t.id == turn).unwrap();
    assert_eq!(answer.text, "Odpowiedź z zapasowego dostawcy.");
    assert_eq!(
        answer.usage.as_ref().map(|u| u.model.as_str()),
        Some(FAKE_MODEL)
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn private_session_uses_only_allowed_routes() {
    let cn =
        fake("deepseek", "Z trasy CN.").with_privacy(ProviderPrivacy::new("cn-may-train", "CN"));
    // Trasa dozwolona w sesji prywatnej wg rejestru zgodności (tag `sg`, jurysdykcja SG|EU).
    let eu = fake("qwen", "Z trasy dozwolonej.").with_privacy(ProviderPrivacy::new("sg", "SG|EU"));
    let mut r = routed(vec![cn.clone(), eu.clone()]).await;
    let core = &r.core;
    let sid = core
        .sessions_create(SessionTemplate::Empty)
        .await
        .unwrap()
        .id;
    // Zwykła sesja: pierwsza trasa z polityki.
    let normal = core
        .turns_send(sid.clone(), send("Pierwsze", None))
        .await
        .unwrap()
        .assistant_turn_id
        .unwrap();
    until(&mut r.rx, ends(&normal)).await;
    assert_eq!(cn.calls().len(), 1);

    let private = core
        .sessions_create(SessionTemplate::Empty)
        .await
        .unwrap()
        .id;
    core.sessions_set_privacy(private.clone(), PrivacyTag::Private)
        .await
        .unwrap();
    for i in 0..3 {
        let turn = core
            .turns_send(private.clone(), send(&format!("Prywatne {i}"), None))
            .await
            .unwrap()
            .assistant_turn_id
            .unwrap();
        let events = until(&mut r.rx, ends(&turn)).await;
        assert_eq!(texts(&events, &turn), "Z trasy dozwolonej.");
    }
    assert_eq!(cn.calls().len(), 1, "sesja prywatna nie trafia do trasy CN");
    assert_eq!(eu.calls().len(), 3);

    // Same trasy niedozwolone → czytelny błąd (nie „brak mózgu").
    let only_cn = routed(vec![
        fake("deepseek", "x").with_privacy(ProviderPrivacy::new("cn-may-train", "CN")),
    ])
    .await;
    let mut rx = only_cn.rx;
    let core = &only_cn.core;
    let sid = core
        .sessions_create(SessionTemplate::Empty)
        .await
        .unwrap()
        .id;
    core.sessions_set_privacy(sid.clone(), PrivacyTag::Private)
        .await
        .unwrap();
    let turn = core
        .turns_send(sid, send("Prywatne", None))
        .await
        .unwrap()
        .assistant_turn_id
        .unwrap();
    let events = until(&mut rx, ends(&turn)).await;
    let error = events
        .iter()
        .find_map(|e| match e {
            AlfaEvent::Error { error, .. } => Some(error.clone()),
            _ => None,
        })
        .unwrap();
    assert_eq!(error.code, TurnErrorCode::Provider);
    assert!(error.message.contains("deepseek"), "{}", error.message);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn local_profile_with_api_keys_needs_local_model() {
    let api = fake("anthropic", "Przez API.");
    let mut r = routed(vec![api.clone()]).await;
    let core = &r.core;
    let sid = core
        .sessions_create(SessionTemplate::Empty)
        .await
        .unwrap()
        .id;
    let turn = core
        .turns_send(
            sid.clone(),
            SendOptions {
                profile: Some(ModelProfile::Local),
                attachments: Vec::new(),
                ..send("Lokalnie", None)
            },
        )
        .await
        .unwrap()
        .assistant_turn_id
        .unwrap();
    let events = until(&mut r.rx, ends(&turn)).await;
    let error = events
        .iter()
        .find_map(|e| match e {
            AlfaEvent::Error { error, .. } => Some(error.clone()),
            _ => None,
        })
        .unwrap();
    assert_eq!(error.code, TurnErrorCode::NoKeys);
    assert_eq!(error.message, NO_LOCAL);
    assert!(api.calls().is_empty(), "profil lokalny nie wysyła do API");
    // Sesja „tylko lokalnie" tak samo.
    core.sessions_set_privacy(sid.clone(), PrivacyTag::LocalOnly)
        .await
        .unwrap();
    let local_only = core
        .turns_send(sid, send("Dalej", Some(turn)))
        .await
        .unwrap()
        .assistant_turn_id
        .unwrap();
    until(&mut r.rx, ends(&local_only)).await;
    assert!(api.calls().is_empty());
    let models = core.models_local_list().await.unwrap();
    assert!(models.iter().any(|m| m.default && !m.installed));
    let unknown = core
        .models_local_download(Some("brak-takiego".into()))
        .await
        .unwrap_err();
    assert_eq!(unknown.code, app_core::ErrorCode::InvalidInput);
}
