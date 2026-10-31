//! Dług katalogu funkcji (docs/plan/45-testy-ai.md): Arena i gałęzie odpowiedzi oraz karty
//! rozmów na rdzeniu z dostawcą atrapy — odpowiedź modelu przypiętego jako nowa gałąź (historia
//! append-only), wybór gałęzi kieruje dalszą rozmowę, karty podają czysty tekst, model i koszt.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::sync::{Arc, Mutex};
use std::time::Duration;

use app_core::dto::{SessionTemplate, Turn};
use app_core::ports::{BrainChoice, BrainError, BrainPort, BrainRequest, HeadlessShell};
use app_core::{AppCore, AppPaths, ErrorCode};
use async_trait::async_trait;
use common::{Harness, ScriptedProvider, TestBrain, ends, harness, options, send, until};
use providers_fake::FAKE_MODEL;

/// „Mózg” atrapy, który zapisuje model zamówiony przez rozmowę (przypięcie Areny albo `auto`).
struct RecordingBrain {
    inner: TestBrain,
    asked: Arc<Mutex<Vec<String>>>,
}

#[async_trait]
impl BrainPort for RecordingBrain {
    async fn choose(&self, request: &BrainRequest) -> Result<BrainChoice, BrainError> {
        if let Some(chat) = &request.chat {
            self.asked.lock().unwrap().push(chat.model.clone());
        }
        self.inner.choose(request).await
    }
    fn keys_configured(&self) -> bool {
        true
    }
}

async fn recording_harness() -> (Harness, Arc<Mutex<Vec<String>>>) {
    let dir = tempfile::tempdir().unwrap();
    let provider = Arc::new(ScriptedProvider::new(Duration::from_millis(1)));
    let shell = Arc::new(HeadlessShell::default());
    let asked = Arc::new(Mutex::new(Vec::new()));
    let mut opts = options(None, shell.clone());
    opts.brain = Some(Arc::new(RecordingBrain {
        inner: TestBrain {
            provider: provider.clone(),
        },
        asked: asked.clone(),
    }));
    let core = AppCore::build(AppPaths::under(dir.path()), opts)
        .await
        .unwrap();
    let rx = core.subscribe_events();
    let h = Harness {
        core,
        provider,
        shell,
        rx,
        dir,
    };
    (h, asked)
}

async fn new_session(h: &Harness) -> String {
    h.core
        .sessions_create(SessionTemplate::Empty)
        .await
        .unwrap()
        .id
}

/// Wiadomość bez wskazanego rodzica (pod aktywną gałęzią); zwraca (tura użytkownika, odpowiedź).
async fn ask(h: &mut Harness, sid: &str, text: &str) -> (String, String) {
    let sent = h
        .core
        .turns_send(sid.into(), send(text, None))
        .await
        .unwrap();
    let answer = sent.assistant_turn_id.unwrap();
    until(&mut h.rx, ends(&answer)).await;
    (sent.user_turn_id, answer)
}

async fn turns(h: &Harness, sid: &str) -> Vec<Turn> {
    h.core.turns_list(sid.into()).await.unwrap().turns
}

fn turn<'a>(all: &'a [Turn], id: &str) -> &'a Turn {
    all.iter().find(|t| t.id == id).unwrap()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn turns_arena_answer_adds_pinned_branch_without_touching_history() {
    let (mut h, asked) = recording_harness().await;
    let sid = new_session(&h).await;
    let (user, first) = ask(&mut h, &sid, "Porównaj dwa modele").await;
    let before = turns(&h, &sid).await;
    let first_text = turn(&before, &first).text.clone();
    let pinned = format!(" fake:{FAKE_MODEL} ");

    // Pod wiadomością użytkownika: nowe dziecko obok pierwszej odpowiedzi.
    let arena = h
        .core
        .turns_arena_answer(sid.clone(), user.clone(), pinned.clone())
        .await
        .unwrap();
    until(&mut h.rx, ends(&arena)).await;
    assert_ne!(arena, first);
    let models = asked.lock().unwrap().clone();
    let qualified = format!("fake:{FAKE_MODEL}");
    assert_eq!(
        models.iter().filter(|m| **m == qualified).count(),
        1,
        "Router dostaje przypięty model raz: {models:?}"
    );
    assert!(
        models.iter().any(|m| *m != qualified),
        "zwykła tura bez przypięcia: {models:?}"
    );
    let after = turns(&h, &sid).await;
    assert_eq!(after.len(), before.len() + 1, "jedna nowa tura");
    assert_eq!(
        turn(&after, &arena).parent_id.as_deref(),
        Some(user.as_str())
    );
    assert_eq!(turn(&after, &first).text, first_text, "append-only");
    assert!(turn(&after, &arena).text.starts_with("Echo:"), "{after:?}");

    // Obok odpowiedzi agentki: rodzeństwo pod tą samą wiadomością.
    let sibling = h
        .core
        .turns_arena_answer(sid.clone(), first.clone(), pinned)
        .await
        .unwrap();
    until(&mut h.rx, ends(&sibling)).await;
    let all = turns(&h, &sid).await;
    assert_eq!(
        turn(&all, &sibling).parent_id.as_deref(),
        Some(user.as_str())
    );
    let answers = all
        .iter()
        .filter(|t| t.parent_id.as_deref() == Some(user.as_str()))
        .count();
    assert_eq!(answers, 3, "trzy gałęzie odpowiedzi pod jednym pytaniem");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn turns_arena_answer_refuses_bad_model_and_foreign_turn() {
    let mut h = harness().await;
    let sid = new_session(&h).await;
    let other = new_session(&h).await;
    let (user, _) = ask(&mut h, &sid, "Pytanie").await;
    let count = turns(&h, &sid).await.len();
    for model in ["auto", "", "bez-dostawcy", "fake:model z spacją"] {
        let err = h
            .core
            .turns_arena_answer(sid.clone(), user.clone(), model.into())
            .await
            .unwrap_err();
        assert_eq!(err.code, ErrorCode::InvalidInput, "{model:?}: {err:?}");
    }
    let foreign = h
        .core
        .turns_arena_answer(other.clone(), user.clone(), format!("fake:{FAKE_MODEL}"))
        .await
        .unwrap_err();
    assert_eq!(foreign.code, ErrorCode::InvalidInput, "tura z innej sesji");
    let missing = h
        .core
        .turns_arena_answer(
            sid.clone(),
            format!("{sid}:t999"),
            format!("fake:{FAKE_MODEL}"),
        )
        .await
        .unwrap_err();
    assert_eq!(missing.code, ErrorCode::NotFound, "{missing:?}");
    assert_eq!(
        turns(&h, &sid).await.len(),
        count,
        "odmowa nic nie dopisuje"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn turns_select_branch_steers_the_next_message() {
    let mut h = harness().await;
    let sid = new_session(&h).await;
    let (user, first) = ask(&mut h, &sid, "Która odpowiedź lepsza?").await;
    let second = h
        .core
        .turns_arena_answer(sid.clone(), user, format!("fake:{FAKE_MODEL}"))
        .await
        .unwrap();
    until(&mut h.rx, ends(&second)).await;

    for chosen in [&first, &second] {
        h.core
            .turns_select_branch(sid.clone(), chosen.clone())
            .await
            .unwrap();
        let (next, _) = ask(&mut h, &sid, "Kontynuuj").await;
        let all = turns(&h, &sid).await;
        assert_eq!(
            turn(&all, &next).parent_id.as_deref(),
            Some(chosen.as_str()),
            "dalsza rozmowa idzie wybraną gałęzią"
        );
    }

    let missing = h
        .core
        .turns_select_branch(sid.clone(), format!("{sid}:t999"))
        .await
        .unwrap_err();
    assert_eq!(missing.code, ErrorCode::NotFound, "{missing:?}");
    for bad in ["t1", "x:y", &format!("{sid}:t0")] {
        let err = h
            .core
            .turns_select_branch(sid.clone(), bad.to_owned())
            .await
            .unwrap_err();
        assert_eq!(err.code, ErrorCode::InvalidInput, "{bad:?}");
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn sessions_cards_show_plain_snippet_model_and_cost() {
    let mut h = harness().await;
    let talk = new_session(&h).await;
    let quiet = new_session(&h).await;
    ask(&mut h, &talk, "**Ważne** pytanie o _raport_").await;

    let cards = h
        .core
        .sessions_cards(vec![
            talk.clone(),
            talk.clone(),
            "  ".into(),
            "nie-ma-takiej-rozmowy".into(),
            quiet.clone(),
        ])
        .await
        .unwrap();
    let ids: Vec<&str> = cards.iter().map(|c| c.session_id.as_str()).collect();
    assert_eq!(
        ids,
        [talk.as_str(), quiet.as_str()],
        "bez powtórzeń i nieznanych"
    );

    let card = &cards[0];
    let snippet = card.snippet.as_deref().unwrap();
    assert!(snippet.contains("Ważne pytanie o raport"), "{snippet}");
    assert!(
        !snippet.contains('*') && !snippet.contains('_'),
        "{snippet}"
    );
    let author = card.snippet_author.as_deref().unwrap();
    assert_ne!(author, "user", "ostatnia jest odpowiedź agentki");
    assert!(card.agents.iter().any(|a| a == author), "{card:?}");
    assert!(
        card.model.as_deref().unwrap().contains(FAKE_MODEL),
        "{card:?}"
    );
    assert!(card.cost.unwrap().minor > 0, "koszt atrapy jest dodatni");

    let empty = &cards[1];
    assert_eq!(
        (&empty.snippet, &empty.model, &empty.cost),
        (&None, &None, &None),
        "rozmowa bez wiadomości"
    );

    // Rozmowa w koszu nie dostaje karty.
    h.core.sessions_remove(quiet.clone()).await.unwrap();
    let after = h.core.sessions_cards(vec![quiet]).await.unwrap();
    assert!(after.is_empty(), "{after:?}");
}
