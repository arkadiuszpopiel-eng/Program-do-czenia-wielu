//! Czytanie na głos w aplikacji (F5) na atrapach: schowek i dokument (UIA `TextPattern`) głosem
//! bieżącej agentki, kolejka zleceń, tempo, przerwanie „stop” głosem (tura nie trafia do czatu)
//! i `Esc`; pole hasła — odmowa.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use app_api::dto::{ReadAction, ReadControlAction, ReadSource, ReadStateView};
use app_api::error::ErrorCode;
use app_api::ports::VoicePort;
use common::{App, RATE, World, app, speech, timeline};
use platform_contract::{ClipboardContent, ClipboardPort, ScreenRect};
use platform_fake::{FakeElement, FakeWindow};

fn rect() -> ScreenRect {
    ScreenRect::from_xywh(100, 100, 800, 600)
}

/// Długi tekst (czytanie trwa dłużej niż scenariusz).
fn long_text(n: usize) -> String {
    (1..=n)
        .map(|i| format!("To jest zdanie numer {i} do przeczytania na głos."))
        .collect::<Vec<_>>()
        .join(" ")
}

fn start(source: ReadSource) -> ReadAction {
    ReadAction::Start { source }
}

fn speaking(a: &App) -> bool {
    a.views
        .lock()
        .unwrap()
        .last()
        .is_some_and(|v| v.read.state == ReadStateView::Speaking)
}

fn idle(a: &App) -> bool {
    a.views
        .lock()
        .unwrap()
        .last()
        .is_some_and(|v| v.read.state == ReadStateView::Idle)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn reading_is_interrupted_by_voice_stop_without_a_chat_turn() {
    let a = app(World::new());
    a.world
        .clipboard
        .set(ClipboardContent::Text(long_text(40)))
        .unwrap();
    let v = a.voice.read(start(ReadSource::Clipboard)).await.unwrap();
    assert_eq!(v.read.state, ReadStateView::Speaking);
    assert_eq!(v.read.agent, "alfa", "głos bieżącej agentki");
    assert_eq!(v.read.app.as_deref(), Some("Schowek"));
    assert_eq!(v.read.segments, 40);
    assert_eq!(v.read.shortcut, "Ctrl+Alt+R");
    a.until("czytanie gra", |a| a.world.audio.recorded_len() > 0)
        .await;
    // „stop” w rozmowie głosowej (przełącznik) przerywa czytanie.
    a.world.stt.script("stop");
    a.world
        .mic(&timeline(4_000, &[(300, speech(120.0, 600, 5, RATE))]));
    a.voice.set_mic_enabled(true).await.unwrap();
    a.until("czytanie przerwane", idle).await;
    assert!(
        a.turns().is_empty(),
        "czytany tekst ani „stop” nie trafiają do czatu"
    );
    a.voice.set_mic_enabled(false).await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn queue_rate_and_escape() {
    let a = app(World::new());
    let w = a.world.desk.add_window(
        FakeWindow::new("Raport — Notatnik", r"C:\Windows\notepad.exe", rect()),
        true,
    );
    a.world.desk.add_element(
        w,
        FakeElement::new("Edytor", "edit", rect())
            .text(&long_text(5))
            .focused(),
    );
    a.world
        .clipboard
        .set(ClipboardContent::Text(long_text(30)))
        .unwrap();
    let v = a
        .voice
        .read(ReadAction::SetRate { rate: 1.5 })
        .await
        .unwrap();
    assert!((v.read.rate - 1.5).abs() < 1e-6);
    let v = a.voice.read(start(ReadSource::Clipboard)).await.unwrap();
    assert!((v.read.rate - 1.5).abs() < 1e-6, "tempo z ustawień");
    let v = a.voice.read(start(ReadSource::Document)).await.unwrap();
    assert_eq!(v.read.queued, 1, "drugie zlecenie czeka w kolejce");
    let v = a
        .voice
        .read(ReadAction::Control {
            control: ReadControlAction::Pause,
        })
        .await
        .unwrap();
    assert!(matches!(
        v.read.state,
        ReadStateView::Paused | ReadStateView::Speaking
    ));
    a.until("pauza", |a| {
        a.views
            .lock()
            .unwrap()
            .last()
            .is_some_and(|v| v.read.state == ReadStateView::Paused)
    })
    .await;
    a.voice
        .read(ReadAction::Control {
            control: ReadControlAction::Resume,
        })
        .await
        .unwrap();
    a.until("wznowione", speaking).await;
    // Esc: stop mowy zatrzymuje czytanie razem z kolejką.
    a.voice.stop_speech().await.unwrap();
    a.until("zatrzymane z kolejką", idle).await;
    let v = a.view().await;
    assert_eq!(v.read.queued, 0);
    // Sterowanie bez czytania — błąd (poza „stop”, który jest bezpieczny zawsze).
    assert!(
        a.voice
            .read(ReadAction::Control {
                control: ReadControlAction::Next
            })
            .await
            .is_err()
    );
    a.voice
        .read(ReadAction::Control {
            control: ReadControlAction::Stop,
        })
        .await
        .unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn password_field_and_empty_clipboard_are_refused() {
    let a = app(World::new());
    let w = a.world.desk.add_window(
        FakeWindow::new("Logowanie", r"C:\Apps\bank.exe", rect()),
        true,
    );
    a.world.desk.add_element(
        w,
        FakeElement::new("Hasło", "edit", rect())
            .password()
            .focused(),
    );
    let e = a.voice.read(start(ReadSource::Document)).await.unwrap_err();
    assert_eq!(e.code, ErrorCode::Forbidden);
    let v = a.view().await;
    assert!(v.read.reason.unwrap().pl.contains("Pole hasła"));
    let e = a
        .voice
        .read(start(ReadSource::Clipboard))
        .await
        .unwrap_err();
    assert_eq!(e.code, ErrorCode::Forbidden);
    assert_eq!(
        a.world.audio.recorded_len(),
        0,
        "nic nie zostało przeczytane"
    );
}
