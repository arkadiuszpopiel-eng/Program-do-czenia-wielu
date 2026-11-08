//! Rozmowa głosowa na atrapach potoku (`voice-pipeline-impl` + `voice-*-fake`, zegar wirtualny):
//! wypowiedź trafia do aktywnej sesji jako tura (append-only), odpowiedź czatu jest mówiona,
//! barge-in przerywa ją, a tura agentki dostaje usłyszany prefiks (także w kontekście kolejnego
//! żądania); pigułka i stan trybu głosowego płyną zdarzeniami.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use app_core::dto::{AlfaEvent, SessionTemplate, VoiceSpeaker, VoiceState};
use app_core::ports::HeadlessShell;
use app_core::{AppCore, AppPaths};
use common::voice::FakeVoice;
use common::*;
use providers_fake::{FAKE_MODEL, Script};

const LINES: [&str; 4] = [
    "Jutro rano masz spotkanie zespołu o dziewiątej.\n",
    "Potem przegląd budżetu z działem finansów.\n",
    "Po południu rozmowa z klientem o nowej umowie.\n",
    "Wieczorem przypomnienie o urodzinach siostry.\n",
];

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn voice_conversation_with_barge_in_records_heard_prefix() {
    let dir = tempfile::tempdir().unwrap();
    let provider = Arc::new(ScriptedProvider::new(Duration::from_millis(1)));
    let shell = Arc::new(HeadlessShell::default());
    let voice = FakeVoice::new();
    voice.mic(16_000, &[(300, 1_200), (6_000, 1_500)]);
    voice.stt.script("Co mam jutro w planie?");
    voice.stt.script("Chodziło mi o wtorek, nie o środę.");
    let mut opts = options(Some(provider.clone()), shell);
    opts.voice_engine = Some(Arc::new(voice.clone()));
    let core = AppCore::build(AppPaths::under(dir.path()), opts)
        .await
        .unwrap();
    let seen: Arc<Mutex<Vec<AlfaEvent>>> = Arc::default();
    let mut rx = core.subscribe_events();
    let sink = seen.clone();
    let collector = tokio::spawn(async move {
        while let Ok(batch) = rx.recv().await {
            let mut s = sink.lock().unwrap();
            s.extend(
                batch
                    .iter()
                    .filter(|e| {
                        matches!(
                            e,
                            AlfaEvent::VoicePill { .. } | AlfaEvent::VoiceStatusChanged { .. }
                        )
                    })
                    .cloned(),
            );
        }
    });
    let sid = core
        .sessions_create(SessionTemplate::Empty)
        .await
        .unwrap()
        .id;
    core.app_set_active_session(Some(sid.clone()))
        .await
        .unwrap();
    provider.push(Script::text(FAKE_MODEL, &LINES));
    provider.push(Script::text(
        FAKE_MODEL,
        &["Rozumiem, we wtorek masz tylko jedno spotkanie.\n"],
    ));
    assert_eq!(core.voice_status().await.unwrap().state, VoiceState::Off);
    core.voice_set_mic_enabled(true).await.unwrap();
    assert_eq!(core.voice_status().await.unwrap().state, VoiceState::Active);

    // Scenariusz do 14 s czasu wirtualnego (limit rzeczywisty 60 s).
    let deadline = Instant::now() + Duration::from_secs(60);
    let turns = loop {
        let snapshot = core.turns_list(sid.clone()).await.unwrap();
        let done = snapshot.turns.len() >= 4
            && snapshot.turns.iter().any(|t| t.heard_prefix.is_some())
            && voice.now() >= 14_000;
        if done || Instant::now() > deadline {
            break snapshot.turns;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    };
    core.voice_set_mic_enabled(false).await.unwrap();
    let texts: Vec<(&str, &str)> = turns
        .iter()
        .map(|t| (t.author.as_str(), t.text.as_str()))
        .collect();
    assert_eq!(turns.len(), 4, "{texts:#?}");
    assert_eq!(turns[0].author, "user");
    assert_eq!(turns[0].text, "Co mam jutro w planie?");
    assert_eq!(turns[2].text, "Chodziło mi o wtorek, nie o środę.");
    // Tura przerwana: pełna treść zostaje (append-only), fakt „usłyszany prefiks" obok.
    let first = &turns[1];
    assert_eq!(first.text.trim_end(), LINES.concat().trim_end());
    let heard = first.heard_prefix.clone().expect("usłyszany prefiks");
    assert!(
        !heard.is_empty() && heard.len() < first.text.len(),
        "{heard:?}"
    );
    assert!(first.text.starts_with(&heard), "{heard:?}");
    let spoken = LINES.map(str::trim).join(" ");
    assert!(
        spoken.starts_with(heard.replace('\n', " ").trim_end()),
        "{heard:?}"
    );
    assert!(turns[3].text.starts_with("Rozumiem"), "{texts:#?}");
    assert_eq!(turns[3].heard_prefix, None);
    // Kolejne żądanie widzi, co użytkownik naprawdę usłyszał.
    let requests = provider.requests();
    assert_eq!(requests.len(), 2, "żądania czatu");
    let context = serde_json::to_string(&requests[1].messages).unwrap();
    let probe: String = heard.chars().take(20).collect();
    assert!(context.contains(&probe), "{context}");

    let deadline = Instant::now() + Duration::from_secs(10);
    while core.voice_status().await.unwrap().state != VoiceState::Off && Instant::now() < deadline {
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    collector.abort();
    let events = seen.lock().unwrap().clone();
    let pills: Vec<_> = events
        .iter()
        .filter_map(|e| match e {
            AlfaEvent::VoicePill { state } => Some(state.clone()),
            _ => None,
        })
        .collect();
    assert!(pills.iter().any(|p| p.speaker == VoiceSpeaker::Agent));
    assert!(pills.iter().any(|p| p.speaker == VoiceSpeaker::User));
    assert!(events.iter().any(|e| matches!(
        e,
        AlfaEvent::VoiceStatusChanged { status } if status.state == VoiceState::Active
    )));
}
