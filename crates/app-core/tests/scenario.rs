//! Pełny scenariusz na atrapach: utwórz sesję → wyślij → strumień → zapis → koszt → wyszukaj →
//! zmień nazwę → ponów (wariant) → edytuj (gałąź) → kontynuuj → stop → usuń z cofnięciem →
//! usunięcie ostateczne (crypto-shredding).

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::time::{Duration, Instant};

use app_core::dto::{
    AlfaEvent, BlockKind, SessionTemplate, StopReason, TimelineFilter, TurnStatus,
};
use common::*;
use providers_fake::{FAKE_MODEL, Script, StreamSpeed};

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn create_send_stream_persist_cost_search_rename_branch_delete() {
    let mut h = harness().await;
    let core = &h.core;
    let session = core.sessions_create(SessionTemplate::Empty).await.unwrap();
    assert_eq!(session.title, "Nowa rozmowa");
    let sid = session.id.clone();

    // Wyślij → strumień → zapis.
    let sent = core
        .turns_send(sid.clone(), send("Policz **koszty** kwartału", None))
        .await
        .unwrap();
    let answer = sent.assistant_turn_id.clone().unwrap();
    let events = until(&mut h.rx, ends(&answer)).await;
    assert_eq!(
        stop_reason(&events, &answer),
        Some(StopReason::End),
        "{events:#?}"
    );
    let deltas: Vec<_> = events
        .iter()
        .filter(|e| matches!(e, AlfaEvent::TextDelta { turn_id, .. } if *turn_id == answer))
        .collect();
    assert!(!deltas.is_empty(), "brak TextDelta");
    assert!(events.iter().any(|e| matches!(e,
        AlfaEvent::TurnAppended { turn, .. } if turn.id == answer && turn.status == TurnStatus::Streaming)));
    let usage = events
        .iter()
        .find_map(|e| match e {
            AlfaEvent::Usage { turn_id, usage, .. } if *turn_id == answer => Some(usage.clone()),
            _ => None,
        })
        .expect("Usage");
    assert!(usage.cost.minor > 0, "koszt w groszach: {usage:?}");
    assert_eq!(usage.model, FAKE_MODEL);

    let snapshot = core.turns_list(sid.clone()).await.unwrap();
    assert_eq!(snapshot.turns.len(), 2);
    let reply = snapshot.turns.iter().find(|t| t.id == answer).unwrap();
    assert_eq!(reply.status, TurnStatus::Complete);
    assert!(reply.text.starts_with("Echo: Policz"));
    assert!(
        reply
            .blocks
            .iter()
            .all(|b| b.closed && b.kind == BlockKind::Text)
    );
    assert!(
        reply.blocks[0]
            .html_sanitized
            .contains("<strong>koszty</strong>")
    );
    assert_eq!(reply.parent_id.as_deref(), Some(sent.user_turn_id.as_str()));
    assert_eq!(reply.usage.as_ref().map(|u| u.cost), Some(usage.cost));

    // Koszt sesji, oś czasu, automatyczny tytuł.
    let costs = core.costs_summary(Some(sid.clone())).await.unwrap();
    assert_eq!(costs.session, usage.cost);
    assert!(costs.month.minor >= usage.cost.minor);
    let timeline = core
        .timeline_list(
            sid.clone(),
            TimelineFilter {
                kinds: vec![],
                min_level: app_core::dto::EventLevel::Trace,
            },
        )
        .await
        .unwrap();
    assert_eq!(timeline.len(), 1);
    assert_eq!(timeline[0].turn_id.as_deref(), Some(answer.as_str()));
    let listed = core.sessions_list().await.unwrap();
    assert_eq!(listed[0].title, "Policz **koszty** kwartału");

    // Wyszukiwanie pełnotekstowe (bez diakrytyków).
    let hits = core.sessions_search("kwartalu".into()).await.unwrap();
    assert!(
        hits.iter()
            .any(|h| h.session_id == sid && h.turn_id.is_some()),
        "{hits:?}"
    );

    // Zmiana nazwy.
    core.sessions_rename(sid.clone(), "Raport Q3".into())
        .await
        .unwrap();
    assert_eq!(core.sessions_list().await.unwrap()[0].title, "Raport Q3");

    // Ponów → wariant (rodzeństwo); edytuj i wyślij → gałąź; kontynuuj → dziecko.
    let variant = core
        .turns_regenerate(sid.clone(), answer.clone(), None)
        .await
        .unwrap();
    until(&mut h.rx, ends(&variant)).await;
    let edited = core
        .turns_edit_and_resend(
            sid.clone(),
            sent.user_turn_id.clone(),
            "Policz przychody".into(),
        )
        .await
        .unwrap();
    let edited_answer = edited.assistant_turn_id.unwrap();
    until(&mut h.rx, ends(&edited_answer)).await;
    let cont = core
        .turns_continue(sid.clone(), edited_answer.clone())
        .await
        .unwrap();
    until(&mut h.rx, ends(&cont)).await;

    let tree = core.turns_list(sid.clone()).await.unwrap().turns;
    assert_eq!(tree.len(), 6, "{tree:#?}");
    let by_id = |id: &str| tree.iter().find(|t| t.id == id).unwrap().clone();
    assert_eq!(
        by_id(&variant).parent_id,
        by_id(&answer).parent_id,
        "wariant = rodzeństwo"
    );
    assert_eq!(
        by_id(&edited.user_turn_id).parent_id,
        None,
        "edycja = nowa gałąź od korzenia"
    );
    assert_eq!(
        by_id(&edited_answer).parent_id.as_deref(),
        Some(edited.user_turn_id.as_str())
    );
    assert_eq!(
        by_id(&cont).continues.as_deref(),
        Some(edited_answer.as_str())
    );
    // Historia nietknięta (append-only): pierwotna odpowiedź bez zmian.
    assert_eq!(by_id(&answer).text, reply.text);
    // Model dostał historię gałęzi edycji, nie pierwotnej wiadomości.
    let last = h.provider.requests().last().cloned().unwrap();
    assert!(last_user_text(&last).contains("Kontynuuj"));
    assert!(
        last.messages
            .iter()
            .all(|m| !format!("{m:?}").contains("Policz **koszty**"))
    );

    // Stop w trakcie wolnego strumienia: ≤ 100 ms (×10 bez ALFA_PERF_BUDGETS).
    let words = "jeden dwa trzy cztery pięć sześć siedem osiem dziewięć dziesięć ".repeat(20);
    h.provider.push(Script::streamed(
        FAKE_MODEL,
        &words,
        StreamSpeed {
            ttft: Duration::from_millis(5),
            tokens_per_sec: 50.0,
        },
    ));
    let slow = core
        .turns_send(sid.clone(), send("Długa odpowiedź", Some(cont.clone())))
        .await
        .unwrap()
        .assistant_turn_id
        .unwrap();
    until(
        &mut h.rx,
        |e| matches!(e, AlfaEvent::TextDelta { turn_id, .. } if *turn_id == slow),
    )
    .await;
    let started = Instant::now();
    core.turns_stop(sid.clone()).await.unwrap();
    let took = started.elapsed();
    eprintln!("anulowanie strumienia: {took:?}");
    assert!(took <= budget(100), "anulowanie trwało {took:?}");
    let events = until(&mut h.rx, ends(&slow)).await;
    assert_eq!(stop_reason(&events, &slow), Some(StopReason::Cancelled));
    let stopped = core.turns_list(sid.clone()).await.unwrap();
    let stopped = stopped.turns.iter().find(|t| t.id == slow).unwrap();
    assert_eq!(stopped.status, TurnStatus::Cancelled);
    assert!(!stopped.text.is_empty(), "część odpowiedzi zachowana");

    // Usuń z cofnięciem → przywróć → usuń ostatecznie.
    let ticket = core.sessions_remove(sid.clone()).await.unwrap();
    assert!(
        core.sessions_list()
            .await
            .unwrap()
            .iter()
            .all(|s| s.id != sid)
    );
    core.sessions_undo_remove(ticket.token).await.unwrap();
    assert!(
        core.sessions_list()
            .await
            .unwrap()
            .iter()
            .any(|s| s.id == sid)
    );
    let db = core.paths().sessions().join(format!("{sid}.db"));
    assert!(db.exists());
    let ticket = core.sessions_remove(sid.clone()).await.unwrap();
    tokio::time::sleep(Duration::from_millis(800)).await;
    assert!(core.sessions_undo_remove(ticket.token).await.is_err());
    assert!(!db.exists(), "baza sesji usunięta (crypto-shredding)");
    assert!(core.turns_list(sid).await.is_err());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn markdown_code_blocks_stream_as_code_and_annotations_persist() {
    let mut h = harness().await;
    let core = &h.core;
    let sid = core
        .sessions_create(SessionTemplate::Coding)
        .await
        .unwrap()
        .id;
    h.provider.push(Script::text(
        FAKE_MODEL,
        &[
            "Kod:\n\n```rust\nfn main",
            "() {}\n```\n\n<script>alert(1)</script>",
        ],
    ));
    let answer = core
        .turns_send(sid.clone(), send("napisz kod", None))
        .await
        .unwrap()
        .assistant_turn_id
        .unwrap();
    let events = until(&mut h.rx, ends(&answer)).await;
    let html: String = events
        .iter()
        .filter_map(|e| match e {
            AlfaEvent::TextDelta { blocks, .. } => Some(
                blocks
                    .iter()
                    .map(|b| b.html_sanitized.clone())
                    .collect::<String>(),
            ),
            _ => None,
        })
        .collect();
    assert!(!html.contains("<script"), "HTML z LLM sanitizowany w Rust");
    let turn = core
        .turns_list(sid.clone())
        .await
        .unwrap()
        .turns
        .into_iter()
        .find(|t| t.id == answer)
        .unwrap();
    let code = turn
        .blocks
        .iter()
        .find(|b| b.kind == BlockKind::Code)
        .unwrap();
    assert_eq!(code.lang.as_deref(), Some("rust"));

    core.turns_rate(answer.clone(), Some(app_core::dto::Rating::Up))
        .await
        .unwrap();
    core.turns_set_hidden(answer.clone(), true).await.unwrap();
    let snap = core.turns_list(sid.clone()).await.unwrap();
    let note = snap.annotations.get(&answer).unwrap();
    assert_eq!(note.rating, Some(app_core::dto::Rating::Up));
    assert!(note.hidden);
    core.turns_rate(answer.clone(), None).await.unwrap();
    let snap = core.turns_list(sid.clone()).await.unwrap();
    assert_eq!(snap.annotations.get(&answer).unwrap().rating, None);

    core.turns_remember(answer.clone(), app_core::dto::RememberScope::Session)
        .await
        .unwrap();
    let err = core
        .turns_remember(answer.clone(), app_core::dto::RememberScope::Global)
        .await
        .unwrap_err();
    assert_eq!(err.code, app_core::ErrorCode::Unavailable);
    let err = core
        .turns_run_code(answer.clone(), code.index)
        .await
        .unwrap_err();
    assert!(err.message.contains("safety-broker"));
}
