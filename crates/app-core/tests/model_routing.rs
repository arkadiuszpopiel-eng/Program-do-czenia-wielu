//! Dobór modelu według trudności (zgłoszenie właściciela 2026-10-14) przez całą ścieżkę rozmowy:
//! prawdziwy adapter i Router do lokalnego serwera udającego dostawcę. Kolejka konta ma model
//! lekki, średni i mocny; łatwe pytanie idzie do lekkiego, wyjaśnienie do średniego, projekt do
//! mocnego. Ustawienia (wyłączenie, strategia, szczebel ręczny) działają od następnej odpowiedzi
//! i przetrwają ponowne uruchomienie (plik `model-routing.json`).

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use app_core::dto::{RoutingLevel, RoutingRung, RoutingStrategy, SessionTemplate, TimelineFilter};
use common::chat_stub::chat_stub;
use common::keys::{core, input};
use common::{ends, send, until};

const KEY: &str = "test-dobor-XXXXXXXXXXXXXXXXXXXXXXX";
const EASY: &str = "Która jest godzina?";
const MEDIUM: &str = "Wyjaśnij, jak działa fotosynteza.";
const HARD: &str = "Zaprojektuj architekturę usługi kolejkowej: najpierw opisz wymagania, \
                    następnie model danych, a potem plan wdrożenia. Uzasadnij wybory.";

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn chat_picks_light_medium_or_strong_model_by_difficulty() {
    let (base, asked) = chat_stub();
    let (core, _dir) = core().await;
    let mut rx = core.subscribe_events();
    let account = core.accounts_add(input(KEY, &base, "praca")).await.unwrap();
    assert!(core.accounts_test(account.id.clone()).await.unwrap().ok);
    core.accounts_set_queue(
        account.id.clone(),
        "chat".into(),
        vec!["gpt-6-sol".into(), "gpt-4o".into(), "gpt-4o-mini".into()],
    )
    .await
    .unwrap();
    let session = core
        .sessions_create(SessionTemplate::Empty)
        .await
        .unwrap()
        .id;
    let world = World {
        core: core.clone(),
        session: session.clone(),
        asked: asked.clone(),
    };
    // Ustawienia domyślne: dobór włączony, zrównoważenie, sprawdzanie na lekkim.
    let routing = core.routing_get().await.unwrap();
    assert!(routing.settings.enabled && routing.settings.escalate);
    assert_eq!(routing.settings.strategy, RoutingStrategy::Balanced);
    assert_eq!(routing.settings.review, RoutingLevel::Easy);
    let rung = |name: &str| {
        routing
            .models
            .iter()
            .find(|m| m.model == name)
            .unwrap_or_else(|| panic!("brak {name}: {:?}", routing.models))
            .rung
    };
    assert_eq!(rung("gpt-4o-mini"), RoutingRung::Light);
    assert_eq!(rung("gpt-4o"), RoutingRung::Standard);
    assert_eq!(rung("gpt-6-sol"), RoutingRung::Strong);

    assert_eq!(world.first(&mut rx, EASY).await, "gpt-4o-mini");
    assert_eq!(world.first(&mut rx, MEDIUM).await, "gpt-4o");
    assert_eq!(world.first(&mut rx, HARD).await, "gpt-6-sol");

    // Oś czasu mówi, dlaczego ten model.
    let timeline = core
        .timeline_list(
            session.clone(),
            TimelineFilter {
                kinds: vec![],
                min_level: app_core::dto::EventLevel::Trace,
            },
        )
        .await
        .unwrap();
    let details: Vec<String> = timeline.iter().filter_map(|e| e.detail.clone()).collect();
    assert!(
        details
            .iter()
            .any(|d| d.contains("dobór: łatwe (krótka, prosta prośba) → szczebel lekki")),
        "{details:?}"
    );
    assert!(
        details.iter().any(|d| d.contains("dobór: trudne (")),
        "{details:?}"
    );

    // Oszczędnie: wyjaśnienie zostaje na lekkim modelu.
    let mut settings = routing.settings.clone();
    settings.strategy = RoutingStrategy::Economy;
    core.routing_set(settings.clone()).await.unwrap();
    assert_eq!(world.first(&mut rx, MEDIUM).await, "gpt-4o-mini");

    // Szczebel ręczny: GPT-4o jako lekki → średnie zadanie idzie szczebel wyżej, do Sola.
    settings.strategy = RoutingStrategy::Balanced;
    settings.rungs.insert(
        format!("{}:gpt-4o", provider_of(&routing.models, "gpt-4o")),
        RoutingRung::Light,
    );
    let saved = core.routing_set(settings.clone()).await.unwrap();
    let gpt4o = saved.models.iter().find(|m| m.model == "gpt-4o").unwrap();
    assert_eq!(gpt4o.rung, RoutingRung::Light);
    assert_eq!(gpt4o.automatic, RoutingRung::Standard);
    assert_eq!(
        world.first(&mut rx, MEDIUM).await,
        "gpt-6-sol",
        "średnie — średniego szczebla już nie ma, więc wyżej"
    );

    // Wyłączony dobór — sama kolejność kolejki konta.
    settings.enabled = false;
    core.routing_set(settings).await.unwrap();
    assert_eq!(world.first(&mut rx, EASY).await, "gpt-6-sol");
}

struct World {
    core: app_core::AppCore,
    session: String,
    asked: std::sync::Arc<std::sync::Mutex<Vec<String>>>,
}

impl World {
    /// Wysyła pytanie i zwraca pierwszy model, o który Router zapytał dostawcę.
    async fn first(
        &self,
        rx: &mut tokio::sync::broadcast::Receiver<app_core::dto::EventBatch>,
        text: &str,
    ) -> String {
        self.asked.lock().unwrap().clear();
        let turn = self
            .core
            .turns_send(self.session.clone(), send(text, None))
            .await
            .unwrap()
            .assistant_turn_id
            .unwrap();
        until(rx, ends(&turn)).await;
        self.asked.lock().unwrap().first().cloned().unwrap()
    }
}

fn provider_of(models: &[app_core::dto::RoutingModel], model: &str) -> String {
    let m = models.iter().find(|m| m.model == model).unwrap();
    m.candidate
        .split_once(':')
        .map(|(p, _)| p.to_owned())
        .unwrap()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn preview_explains_the_choice_and_bad_input_is_refused() {
    let (base, _asked) = chat_stub();
    let (core, _dir) = core().await;
    let account = core.accounts_add(input(KEY, &base, "praca")).await.unwrap();
    assert!(core.accounts_test(account.id.clone()).await.unwrap().ok);
    core.accounts_set_queue(
        account.id.clone(),
        "code".into(),
        vec!["gpt-6-sol".into(), "gpt-4o".into(), "gpt-4o-mini".into()],
    )
    .await
    .unwrap();

    let easy = core
        .routing_preview(EASY.into(), "chat".into())
        .await
        .unwrap();
    assert_eq!(easy.difficulty, Some(RoutingLevel::Easy));
    assert_eq!(easy.start, Some(RoutingRung::Light));
    assert_eq!(easy.reasons, "krótka, prosta prośba");

    // Kod: prosta prośba nie schodzi poniżej średniego; kolejność kolejki konta „Kod”.
    let code = core
        .routing_preview("dodaj komentarz".into(), "code".into())
        .await
        .unwrap();
    assert_eq!(code.difficulty, Some(RoutingLevel::Medium));
    assert!(code.reasons.contains("zadanie kodu"), "{}", code.reasons);
    let order: Vec<&str> = code.order.iter().map(|m| m.model.as_str()).collect();
    assert_eq!(order, ["gpt-4o", "gpt-6-sol", "gpt-4o-mini"]);
    assert!(code.order.iter().all(|m| m.provider_name == "OpenAI"));

    let hard = core
        .routing_preview(HARD.into(), "code".into())
        .await
        .unwrap();
    assert_eq!(hard.difficulty, Some(RoutingLevel::Hard));
    assert_eq!(hard.order[0].model, "gpt-6-sol");
    assert!(hard.score.unwrap() >= 58);

    // Głos-szybka — bez doboru (czas odpowiedzi).
    let voice = core
        .routing_preview(HARD.into(), "voice_fast".into())
        .await
        .unwrap();
    assert!(voice.difficulty.is_none());

    let bad_class = core
        .routing_preview("x".into(), "rozmowa".into())
        .await
        .unwrap_err();
    assert!(
        bad_class.message.contains("klasa zadań"),
        "{}",
        bad_class.message
    );
    let too_long = core
        .routing_preview("x".repeat(20_001), "chat".into())
        .await
        .unwrap_err();
    assert!(too_long.message.contains("za długi"));
    let mut settings = core.routing_get().await.unwrap().settings;
    settings
        .rungs
        .insert("bez-dwukropka".into(), RoutingRung::Strong);
    let bad_key = core.routing_set(settings).await.unwrap_err();
    assert!(
        bad_key.message.contains("dostawca:model"),
        "{}",
        bad_key.message
    );
}
