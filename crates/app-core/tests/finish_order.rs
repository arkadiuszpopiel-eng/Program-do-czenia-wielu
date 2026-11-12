//! Zdarzenie terminalne nie może wyprzedzać faktów zakończonej generacji.
#![allow(clippy::unwrap_used, clippy::expect_used)]
mod common;

use app_core::dto::{AlfaEvent, SessionTemplate, TimelineKind, WorkdirChoice};
use common::{ends, harness, send, until};
use providers_fake::{FAKE_MODEL, Script};

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn stop_follows_persisted_model_timeline_and_generation_cleanup() {
    let mut h = harness().await;
    let sid = h
        .core
        .sessions_create(SessionTemplate::Empty)
        .await
        .unwrap()
        .id;
    h.core
        .sessions_choose_workdir(sid.clone(), WorkdirChoice::None)
        .await
        .unwrap();
    h.provider
        .push(Script::text(FAKE_MODEL, &["Odpowiedź testowa."]));
    let turn = h
        .core
        .turns_send(sid, send("Pytanie testowe", None))
        .await
        .unwrap()
        .assistant_turn_id
        .unwrap();
    let events = until(&mut h.rx, ends(&turn)).await;
    let terminal = events
        .iter()
        .position(|e| matches!(e,AlfaEvent::Stop{turn_id,..}if turn_id==&turn))
        .unwrap();
    let timeline = events.iter().position(|e| matches!(e,AlfaEvent::TimelineAppended{event}if event.turn_id.as_deref()==Some(turn.as_str())&&event.kind==TimelineKind::ModelCall));
    assert!(
        timeline.is_some_and(|p| p < terminal),
        "Stop poprzedził oś czasu zakończonego wywołania: timeline={timeline:?}, Stop={terminal}"
    );
    assert_eq!(
        h.core
            .system_kill_all(app_core::ports::KillOrigin::Ui)
            .await,
        0,
        "Po Stop nie ma już aktywnej generacji tej próby"
    );
}
