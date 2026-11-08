//! Dobór modelu według trudności (zgłoszenie właściciela 2026-10-14): łatwe → lekki model, kod →
//! model średni (lokalny Qwen 27B), trudne → mocny (Opus, GPT Sol); sprawdzanie na lekkim;
//! po odrzuceniu odpowiedzi — szczebel wyżej; przypięcie i klasy bez doboru bez zmian.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod support;

use std::collections::BTreeMap;
use std::sync::Arc;

use providers_contract::{ChatRequest, Difficulty, Message, RouteHint, Strategy};
use providers_fake::FakeProvider;
use router_contract::{
    Candidate, Constraints, DifficultyPolicy, RouteKind, RoutePolicy, Router, Rung, TaskClass,
};
use router_impl::{RoutedProvider, RouterCore};
use support::*;

const OPUS: &str = "claude-opus-5-5";
const SOL: &str = "gpt-6-sol";
const LUNA: &str = "gpt-6-luna";
const QWEN: &str = "qwen3-27b-instruct-q4_k_m";
const BIELIK: &str = "bielik-4.5b-v3.0-instruct-q8_0";

fn c(provider: &str, model: &str) -> Candidate {
    Candidate::new(provider, model)
}

struct Setup {
    core: Arc<RouterCore>,
    anthropic: FakeProvider,
    local: FakeProvider,
    light: FakeProvider,
}

/// Opus i GPT (mocne i średni), lokalny Qwen 27B (średni) i lekki Bielik na karcie zintegrowanej.
fn setup() -> Setup {
    let core = Arc::new(RouterCore::default());
    let anthropic = fake("anthropic");
    let local = fake("local");
    let light = fake("local-light");
    core.register(Arc::new(anthropic.clone()), RouteKind::Api);
    core.register(Arc::new(fake("openai")), RouteKind::Api);
    core.register(Arc::new(local.clone()), RouteKind::Local);
    core.register(Arc::new(light.clone()), RouteKind::Local);
    core.set_light_local(Some(providers_contract::ProviderId::new("local-light")));
    let list = vec![
        c("anthropic", OPUS),
        c("openai", SOL),
        c("openai", LUNA),
        c("local", QWEN),
        c("local-light", BIELIK),
    ];
    let mut policy = RoutePolicy::defaults(None, &[]);
    for class in [
        TaskClass::Conversation,
        TaskClass::Code,
        TaskClass::Planning,
        TaskClass::VoiceFast,
    ] {
        policy.classes.insert(class, list.clone());
    }
    core.set_policy(Some(policy));
    Setup {
        core,
        anthropic,
        local,
        light,
    }
}

fn ask(text: &str) -> ChatRequest {
    ChatRequest::new("auto", vec![Message::user_text(text)])
}

fn with_hint(text: &str, hint: RouteHint) -> ChatRequest {
    let mut r = ask(text);
    r.meta.route = hint;
    r
}

fn order(core: &RouterCore, class: TaskClass, r: &ChatRequest) -> Vec<String> {
    let d = core
        .decide(class, &Constraints::from_request(class, r), Some(r))
        .unwrap();
    d.targets().map(|t| t.model.clone()).collect()
}

#[test]
fn escalation_in_shared_queue_skips_failed_prefix_instead_of_repinning_it() {
    let s = setup();
    s.core.set_global_queues(BTreeMap::from([(
        TaskClass::Conversation,
        vec![c("openai", LUNA), c("anthropic", OPUS), c("openai", SOL)],
    )]));
    let mut req = ask("Zadanie wymaga mocniejszego modelu");
    req.model = format!("openai:{LUNA}");
    req.meta.route.escalate_from = Some(format!("openai:{LUNA}"));
    assert_eq!(order(&s.core, TaskClass::Conversation, &req), [OPUS, SOL]);
}

const EASY: &str = "Która jest godzina?";
const HARD: &str = "Zaprojektuj architekturę usługi kolejkowej: najpierw opisz wymagania, \
                    następnie model danych, a potem plan wdrożenia. Uzasadnij wybory.";

#[test]
fn easy_question_goes_to_light_local_model_with_everything_else_as_fallback() {
    let s = setup();
    assert_eq!(
        order(&s.core, TaskClass::Conversation, &ask(EASY)),
        [BIELIK, QWEN, LUNA, OPUS, SOL]
    );
    let d = s
        .core
        .decide(
            TaskClass::Conversation,
            &Constraints::from_request(TaskClass::Conversation, &ask(EASY)),
            Some(&ask(EASY)),
        )
        .unwrap();
    let note = d.difficulty.unwrap();
    assert_eq!(note.difficulty, Difficulty::Easy);
    assert_eq!(note.start, Rung::Light);
    assert_eq!(note.reasons, "krótka, prosta prośba");
}

#[test]
fn code_starts_on_medium_rung_local_first_then_strong_models() {
    let s = setup();
    // Prosta prośba w klasie „Kod” nie schodzi poniżej średniego szczebla.
    assert_eq!(
        order(&s.core, TaskClass::Code, &ask("dodaj komentarz")),
        [QWEN, LUNA, OPUS, SOL, BIELIK]
    );
    let r = ask("dodaj komentarz");
    let note = s
        .core
        .judge(
            TaskClass::Code,
            &Constraints::from_request(TaskClass::Code, &r),
            Some(&r),
        )
        .unwrap();
    assert_eq!(note.difficulty, Difficulty::Medium);
    assert!(note.reasons.ends_with("; zadanie kodu"), "{}", note.reasons);
}

#[test]
fn hard_task_starts_on_strong_models() {
    let s = setup();
    assert_eq!(
        order(&s.core, TaskClass::Conversation, &ask(HARD)),
        [OPUS, SOL, QWEN, LUNA, BIELIK]
    );
}

#[test]
fn review_runs_on_light_rung_and_rejection_escalates_one_rung_up() {
    let s = setup();
    let review = RouteHint {
        review: true,
        ..RouteHint::default()
    };
    assert_eq!(
        order(&s.core, TaskClass::Code, &with_hint(HARD, review)),
        [BIELIK, QWEN, LUNA, OPUS, SOL],
        "sprawdzanie — lekki model, mimo klasy „Kod” i trudnej treści"
    );
    // Odpowiedź Qwena odrzucona → poprawka zaczyna od mocnych, Qwen na końcu.
    let fix = RouteHint {
        difficulty: Some(Difficulty::Medium),
        escalate_from: Some(format!("local:{QWEN}")),
        ..RouteHint::default()
    };
    assert_eq!(
        order(&s.core, TaskClass::Code, &with_hint("popraw", fix.clone())),
        [OPUS, SOL, LUNA, BIELIK, QWEN]
    );
    // Odrzucony mocny model → inny mocny, potem reszta, odrzucony na końcu.
    let again = RouteHint {
        escalate_from: Some(format!("anthropic:{OPUS}")),
        ..fix.clone()
    };
    assert_eq!(
        order(&s.core, TaskClass::Code, &with_hint("popraw", again)),
        [SOL, QWEN, LUNA, BIELIK, OPUS]
    );
    // Podnoszenie wyłączone w ustawieniach — zwykła kolejność średniego szczebla.
    s.core.set_difficulty_policy(DifficultyPolicy {
        escalate: false,
        ..DifficultyPolicy::default()
    });
    assert_eq!(
        order(&s.core, TaskClass::Code, &with_hint("popraw", fix)),
        [QWEN, LUNA, OPUS, SOL, BIELIK]
    );
}

#[test]
fn strategy_manual_rungs_and_api_first_ties() {
    let s = setup();
    let explain = ask("Wyjaśnij, jak działa fotosynteza.");
    assert_eq!(
        order(&s.core, TaskClass::Conversation, &explain)[0],
        QWEN,
        "zrównoważenie: średnie"
    );
    s.core.set_difficulty_policy(DifficultyPolicy {
        strategy: Strategy::Economy,
        ..DifficultyPolicy::default()
    });
    assert_eq!(
        order(&s.core, TaskClass::Conversation, &explain)[0],
        BIELIK,
        "oszczędnie: łatwe"
    );
    s.core.set_difficulty_policy(DifficultyPolicy {
        local_first: false,
        rungs: BTreeMap::from([(format!("openai:{LUNA}"), Rung::Strong)]),
        ..DifficultyPolicy::default()
    });
    assert_eq!(s.core.rung_of(&c("openai", LUNA)), Rung::Strong);
    assert_eq!(s.core.automatic_rung(&c("openai", LUNA)), Rung::Standard);
    assert_eq!(
        order(&s.core, TaskClass::Conversation, &ask(HARD)),
        [OPUS, SOL, LUNA, QWEN, BIELIK],
        "ręczny szczebel GPT Luna = mocny; remis bez „najpierw lokalny” — kolejność kolejki"
    );
}

#[test]
fn pinned_model_disabled_policy_and_voice_keep_their_order() {
    let s = setup();
    let mut r = ask(EASY);
    r.model = format!("anthropic:{OPUS}");
    assert_eq!(
        order(&s.core, TaskClass::Conversation, &r),
        [OPUS, BIELIK, QWEN, LUNA, SOL]
    );
    assert_eq!(
        order(&s.core, TaskClass::VoiceFast, &ask(HARD)),
        [OPUS, SOL, LUNA, QWEN, BIELIK],
        "głos-szybka bez doboru"
    );
    s.core.set_difficulty_policy(DifficultyPolicy {
        enabled: false,
        ..DifficultyPolicy::default()
    });
    assert_eq!(
        order(&s.core, TaskClass::Conversation, &ask(EASY)),
        [OPUS, SOL, LUNA, QWEN, BIELIK]
    );
    let none = s
        .core
        .decide(TaskClass::Conversation, &Constraints::default(), None)
        .unwrap();
    assert!(none.difficulty.is_none());
}

#[tokio::test(start_paused = true)]
async fn routed_provider_answers_with_the_chosen_rung_and_role_class() {
    let s = setup();
    let chat = RoutedProvider::new(s.core.clone(), TaskClass::Conversation);
    assert_eq!(
        text(&collect(&chat, ask(EASY)).await),
        "local-light odpowiada"
    );
    assert_eq!(s.light.requests().last().unwrap().model, BIELIK);
    assert_eq!(
        text(&collect(&chat, ask(HARD)).await),
        "anthropic odpowiada"
    );
    assert_eq!(s.anthropic.requests().last().unwrap().model, OPUS);
    // Delegacja do Koderki: klasa roli ze wskazówki → „Kod” (średni szczebel).
    let code = RouteHint {
        class: Some("code".into()),
        ..RouteHint::default()
    };
    assert_eq!(
        text(&collect(&chat, with_hint("dodaj komentarz", code)).await),
        "local odpowiada"
    );
    assert_eq!(s.local.requests().last().unwrap().model, QWEN);
    // Decyzja na zdarzeniu niesie uzasadnienie doboru.
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    s.core.set_event_sink(Some(tx));
    let _ = s
        .core
        .route(
            TaskClass::Conversation,
            &Constraints::from_request(TaskClass::Conversation, &ask(HARD)),
            Some(&ask(HARD)),
        )
        .unwrap();
    let Some(router_contract::RouterEvent::Decision { decision }) = rx.recv().await else {
        panic!("brak decyzji");
    };
    let note = decision.difficulty.unwrap();
    assert_eq!(note.difficulty, Difficulty::Hard);
    assert!(note.summary().starts_with("trudne ("), "{}", note.summary());
}

#[test]
fn manual_queue_keeps_its_order_unless_reordering_is_allowed() {
    let s = setup();
    // Kolejka konta dla rozmowy ustawiona ręcznie → dobór nie zmienia kolejności.
    s.core.set_provider_queues(BTreeMap::from([(
        providers_contract::ProviderId::new("openai"),
        BTreeMap::from([(TaskClass::Conversation, vec![SOL.to_owned()])]),
    )]));
    assert!(s.core.manual_queue(TaskClass::Conversation));
    assert!(!s.core.manual_queue(TaskClass::Code));
    assert_eq!(
        order(&s.core, TaskClass::Conversation, &ask(EASY)),
        [OPUS, SOL, LUNA, QWEN, BIELIK]
    );
    let r = ask(EASY);
    let note = s
        .core
        .judge(
            TaskClass::Conversation,
            &Constraints::from_request(TaskClass::Conversation, &r),
            Some(&r),
        )
        .unwrap();
    assert!(note.kept_queue && !note.reorders());
    assert!(
        note.summary().contains("kolejka ustawiona ręcznie"),
        "{}",
        note.summary()
    );
    // Klasa bez kolejki — dobór działa.
    assert_eq!(
        order(&s.core, TaskClass::Code, &ask("dodaj komentarz"))[0],
        QWEN
    );
    // Podniesienie po odrzuceniu działa także przy ręcznej kolejce.
    let fix = with_hint(
        "popraw",
        RouteHint {
            escalate_from: Some(format!("local:{QWEN}")),
            ..RouteHint::default()
        },
    );
    assert_eq!(
        order(&s.core, TaskClass::Conversation, &fix),
        [OPUS, SOL, LUNA, BIELIK, QWEN]
    );
    // Włączone „zmieniaj też kolejki ręczne”.
    s.core.set_difficulty_policy(DifficultyPolicy {
        reorder_queues: true,
        ..DifficultyPolicy::default()
    });
    assert_eq!(
        order(&s.core, TaskClass::Conversation, &ask(EASY))[0],
        BIELIK
    );
}

#[test]
fn can_escalate_only_when_a_stronger_model_is_queued() {
    use providers_contract::ModelProvider;
    let s = setup();
    let chat = RoutedProvider::new(s.core.clone(), TaskClass::Conversation);
    assert!(
        chat.can_escalate(&format!("local:{QWEN}")),
        "Qwen → są mocne"
    );
    assert!(chat.can_escalate(&format!("local-light:{BIELIK}")));
    assert!(
        !chat.can_escalate(&format!("anthropic:{OPUS}")),
        "nad mocnym nic"
    );
    assert!(!chat.can_escalate("bez-dwukropka"));
    s.core.set_difficulty_policy(DifficultyPolicy {
        escalate: false,
        ..DifficultyPolicy::default()
    });
    assert!(
        !chat.can_escalate(&format!("local:{QWEN}")),
        "podnoszenie wyłączone"
    );
}
