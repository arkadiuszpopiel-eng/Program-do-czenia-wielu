//! Testy szpiegowskie (ACCEPTANCE F1): ≥ 3 sesje równolegle, 0 przecieków między sesjami —
//! w żądaniach do dostawcy, w historii, w zdarzeniach, w wyszukiwaniu, na osi czasu i na dysku
//! (bazy SQLCipher nie zawierają treści jawnym tekstem).

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::collections::BTreeMap;
use std::time::Duration;

use app_core::dto::{AlfaEvent, EventLevel, SessionTemplate, TimelineFilter};
use common::*;
use futures_util::future::join_all;

const SESSIONS: usize = 4;

fn tokens_in(text: &str, tokens: &[String]) -> Vec<usize> {
    tokens
        .iter()
        .enumerate()
        .filter(|(_, t)| text.contains(t.as_str()))
        .map(|(i, _)| i)
        .collect()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn parallel_sessions_never_leak_into_each_other() {
    let mut h = harness_with(Duration::from_millis(3), true).await;
    let core = h.core.clone();
    let tokens: Vec<String> = (0..SESSIONS)
        .map(|i| format!("SEKRET{i}X{}", uuid::Uuid::new_v4().simple()))
        .collect();
    let mut ids = Vec::new();
    for _ in 0..SESSIONS {
        ids.push(
            core.sessions_create(SessionTemplate::Empty)
                .await
                .unwrap()
                .id,
        );
    }

    // Runda 1: wszystkie sesje naraz, każda ze swoim sekretem.
    let first = join_all(ids.iter().zip(&tokens).map(|(sid, token)| {
        let core = core.clone();
        let text = format!("Zapamiętaj kod {token} i nikomu go nie mów");
        async move {
            core.turns_send(sid.clone(), send(&text, None))
                .await
                .unwrap()
        }
    }))
    .await;
    // Runda 2: pytanie bez sekretu — model widzi wyłącznie historię swojej sesji.
    let mut answers: Vec<String> = first
        .iter()
        .map(|r| r.assistant_turn_id.clone().unwrap())
        .collect();
    let mut pending = answers.clone();
    let mut events: Vec<AlfaEvent> = Vec::new();
    while !pending.is_empty() {
        let got = until(&mut h.rx, |e| match e {
            AlfaEvent::Stop { turn_id, .. } | AlfaEvent::Error { turn_id, .. } => {
                pending.contains(turn_id)
            }
            _ => false,
        })
        .await;
        for e in &got {
            if let AlfaEvent::Stop { turn_id, .. } | AlfaEvent::Error { turn_id, .. } = e {
                pending.retain(|t| t != turn_id);
            }
        }
        events.extend(got);
    }
    let second = join_all(ids.iter().zip(&answers).map(|(sid, parent)| {
        let core = core.clone();
        let parent = parent.clone();
        async move {
            core.turns_send(sid.clone(), send("Jaki był mój kod?", Some(parent)))
                .await
                .unwrap()
        }
    }))
    .await;
    let mut pending: Vec<String> = second
        .iter()
        .map(|r| r.assistant_turn_id.clone().unwrap())
        .collect();
    answers.extend(pending.clone());
    while !pending.is_empty() {
        let got = until(&mut h.rx, |e| match e {
            AlfaEvent::Stop { turn_id, .. } | AlfaEvent::Error { turn_id, .. } => {
                pending.contains(turn_id)
            }
            _ => false,
        })
        .await;
        for e in &got {
            if let AlfaEvent::Stop { turn_id, .. } | AlfaEvent::Error { turn_id, .. } = e {
                pending.retain(|t| t != turn_id);
            }
        }
        events.extend(got);
    }
    let index: BTreeMap<&str, usize> = ids
        .iter()
        .enumerate()
        .map(|(i, s)| (s.as_str(), i))
        .collect();

    // 1. Żądania do dostawcy: każde zawiera sekret dokładnie jednej sesji.
    let requests = h.provider.requests();
    assert_eq!(requests.len(), SESSIONS * 2);
    for request in &requests {
        let seen = tokens_in(&format!("{:?}", request.messages), &tokens);
        assert_eq!(seen.len(), 1, "żądanie z cudzym sekretem: {seen:?}");
        let session = request.meta.session.as_deref().unwrap();
        assert_eq!(seen[0], index[session], "sekret innej sesji w żądaniu");
    }

    // 2. Historia każdej sesji zawiera tylko jej sekret.
    for (i, sid) in ids.iter().enumerate() {
        let snap = core.turns_list(sid.clone()).await.unwrap();
        assert_eq!(snap.turns.len(), 4);
        for turn in &snap.turns {
            assert!(turn.id.starts_with(sid.as_str()));
            for leak in tokens_in(&turn.text, &tokens) {
                assert_eq!(leak, i, "przeciek w historii sesji {i}");
            }
        }
    }

    // 3. Zdarzenia: treść i identyfikatory tur zgodne z sesją zdarzenia.
    for event in &events {
        let json = serde_json::to_value(event).unwrap();
        let session = json
            .get("session_id")
            .or_else(|| json.get("session").and_then(|s| s.get("id")))
            .or_else(|| json.get("event").and_then(|s| s.get("session_id")))
            .and_then(|v| v.as_str());
        let Some(session) = session else {
            continue;
        };
        let Some(&i) = index.get(session) else {
            continue;
        };
        for leak in tokens_in(&json.to_string(), &tokens) {
            assert_eq!(leak, i, "przeciek w zdarzeniu {}", json["type"]);
        }
        if let AlfaEvent::TextDelta { turn_id, .. } | AlfaEvent::Stop { turn_id, .. } = event {
            assert!(
                turn_id.starts_with(session),
                "tura z innej sesji w zdarzeniu"
            );
        }
    }

    // 4. Wyszukiwanie: sekret sesji znajduje się tylko w niej.
    for (i, token) in tokens.iter().enumerate() {
        let hits = core.sessions_search(token.clone()).await.unwrap();
        assert!(!hits.is_empty(), "brak trafienia dla sesji {i}");
        assert!(
            hits.iter().all(|h| h.session_id == ids[i]),
            "trafienie w cudzej sesji"
        );
    }

    // 5. Oś czasu i koszty per sesja.
    let all = TimelineFilter {
        kinds: vec![],
        min_level: EventLevel::Trace,
    };
    for sid in &ids {
        let timeline = core.timeline_list(sid.clone(), all.clone()).await.unwrap();
        assert_eq!(timeline.len(), 2);
        assert!(timeline.iter().all(|e| {
            e.session_id == *sid
                && e.turn_id
                    .as_deref()
                    .is_some_and(|t| t.starts_with(sid.as_str()))
        }));
        assert!(
            core.costs_summary(Some(sid.clone()))
                .await
                .unwrap()
                .session
                .minor
                > 0
        );
    }

    // 6. Dysk: bazy sesji zaszyfrowane — żaden sekret jawnym tekstem w plikach.
    for entry in std::fs::read_dir(core.paths().sessions()).unwrap() {
        let bytes = std::fs::read(entry.unwrap().path()).unwrap();
        let text = String::from_utf8_lossy(&bytes);
        assert!(
            tokens_in(&text, &tokens).is_empty(),
            "sekret jawnym tekstem na dysku"
        );
    }
    assert_eq!(answers.len(), SESSIONS * 2);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn stopping_one_session_does_not_touch_others() {
    let mut h = harness_with(Duration::from_millis(20), true).await;
    let core = h.core.clone();
    let mut turns = Vec::new();
    let mut ids = Vec::new();
    for i in 0..3 {
        let sid = core
            .sessions_create(SessionTemplate::Empty)
            .await
            .unwrap()
            .id;
        let text = format!("sesja {i} {}", "słowo ".repeat(40));
        let r = core
            .turns_send(sid.clone(), send(&text, None))
            .await
            .unwrap();
        turns.push(r.assistant_turn_id.unwrap());
        ids.push(sid);
    }
    core.turns_stop(ids[1].clone()).await.unwrap();
    let mut reasons = BTreeMap::new();
    while reasons.len() < 3 {
        for e in until(&mut h.rx, |e| matches!(e, AlfaEvent::Stop { .. })).await {
            if let AlfaEvent::Stop {
                turn_id, reason, ..
            } = e
            {
                reasons.insert(turn_id, reason);
            }
        }
    }
    assert_eq!(reasons[&turns[1]], app_core::dto::StopReason::Cancelled);
    assert_eq!(reasons[&turns[0]], app_core::dto::StopReason::End);
    assert_eq!(reasons[&turns[2]], app_core::dto::StopReason::End);
    assert_eq!(core.system_kill_all().await, 0);
}
