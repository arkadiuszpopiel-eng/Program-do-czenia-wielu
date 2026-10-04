//! Dyktowanie w aplikacji (F5) na `platform-fake` i wirtualnym zegarze: cel = okno na pierwszym
//! planie, tekst przez `InputPort`, **pole hasła = odmowa** (nic nie zostaje wpisane), profile
//! per aplikacja, podgląd ostatniej frazy tylko w trakcie sesji.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use app_api::dto::{DictationAction, DictationProfile, DictationStateView};
use app_api::error::ErrorCode;
use app_api::ports::VoicePort;
use common::{App, RATE, World, app, speech, timeline};
use platform_contract::{ScreenRect, WindowId};
use platform_fake::{FakeElement, FakeWindow, GuiRecordKind};

fn rect() -> ScreenRect {
    ScreenRect::from_xywh(100, 100, 800, 600)
}

fn number(f: &str, key: &str) -> Option<u32> {
    let start = f.find(key)? + key.len();
    f[start..]
        .chars()
        .take_while(char::is_ascii_digit)
        .collect::<String>()
        .parse()
        .ok()
}

/// Tekst pola odtworzony z dostarczonych zdarzeń wejścia (Unicode, Enter, Backspace).
fn typed(a: &App, window: WindowId) -> String {
    let mut units: Vec<u16> = Vec::new();
    for r in a
        .world
        .desk
        .records()
        .into_iter()
        .filter(|r| r.window == window)
    {
        let GuiRecordKind::Input(e) = r.kind else {
            continue;
        };
        if e.contains("up: true") {
            continue;
        }
        if e.starts_with("Unicode") {
            units.extend(number(&e, "unit: ").and_then(|u| u16::try_from(u).ok()));
        } else if e.starts_with("Key") {
            match number(&e, "vk: ") {
                Some(0x0D) => units.push(0x0A),
                Some(0x08) => {
                    units.pop();
                }
                _ => {}
            }
        }
    }
    String::from_utf16_lossy(&units)
}

fn notepad(a: &App) -> WindowId {
    let w = a.world.desk.add_window(
        FakeWindow::new("Bez tytułu — Notatnik", r"C:\Windows\notepad.exe", rect()),
        true,
    );
    a.world
        .desk
        .add_element(w, FakeElement::new("Edytor", "edit", rect()).focused());
    w
}

fn mic_phrase(a: &App) {
    a.world
        .mic(&timeline(6_000, &[(500, speech(120.0, 1_500, 3, RATE))]));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn dictation_into_a_password_field_is_refused() {
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
    a.world.dict_stt.script("moje tajne hasło");
    mic_phrase(&a);
    let e = a.voice.dictation(DictationAction::Start).await.unwrap_err();
    assert_eq!(e.code, ErrorCode::Forbidden);
    assert!(e.message.contains("Pole hasła"), "{}", e.message);
    let v = a.view().await;
    assert_eq!(v.dictation.state, DictationStateView::Idle);
    assert!(v.dictation.reason.unwrap().pl.contains("Pole hasła"));
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    assert_eq!(typed(&a, w), "", "nic nie wpisano do pola hasła");
    assert_eq!(a.world.desk.injected_batches(), 0);
    assert_eq!(
        a.world.dict_stt.pending_script(),
        1,
        "audio nie trafiło do STT"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn dictation_types_into_the_foreground_app_and_stops() {
    let a = app(World::new());
    let w = notepad(&a);
    a.world.dict_stt.script("dzień dobry kropka");
    mic_phrase(&a);
    let v = a.voice.dictation(DictationAction::Start).await.unwrap();
    assert_eq!(v.dictation.state, DictationStateView::Active);
    assert_eq!(v.dictation.shortcut, "Ctrl+Alt+D");
    a.until("tekst wpisany", |a| typed(a, w) == "Dzień dobry.")
        .await;
    a.until("podgląd i liczba znaków", |a| {
        a.views.lock().unwrap().iter().any(|v| {
            v.dictation.preview.as_deref() == Some("dzień dobry kropka")
                && v.dictation.typed_chars > 0
        })
    })
    .await;
    // W trakcie dyktowania tury głosowe nie idą do czatu (strażnik), czat nic nie dostał.
    assert!(a.turns().is_empty());
    a.voice.dictation(DictationAction::Stop).await.unwrap();
    a.until("koniec sesji", |a| {
        a.views
            .lock()
            .unwrap()
            .last()
            .is_some_and(|v| v.dictation.state == DictationStateView::Idle)
    })
    .await;
    let v = a.view().await;
    assert_eq!(v.dictation.preview, None, "podgląd czyszczony po sesji");
    assert_eq!(a.world.audio.open_inputs(), 0, "mikrofon zamknięty");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn app_profile_changes_capitalization() {
    let a = app(World::new());
    let e = a
        .voice
        .dictation(DictationAction::SaveProfile {
            profile: DictationProfile {
                app: "notepad".into(),
                capitalize_start: false,
                block_enter: true,
            },
        })
        .await
        .unwrap_err();
    assert_eq!(e.code, ErrorCode::InvalidInput);
    let v = a
        .voice
        .dictation(DictationAction::SaveProfile {
            profile: DictationProfile {
                app: r"C:\Windows\NOTEPAD.EXE".into(),
                capitalize_start: false,
                block_enter: true,
            },
        })
        .await
        .unwrap();
    assert_eq!(v.dictation.profiles.len(), 1);
    assert_eq!(v.dictation.profiles[0].app, "notepad.exe");
    let w = notepad(&a);
    a.world.dict_stt.script("dzień dobry kropka");
    mic_phrase(&a);
    a.voice.dictation(DictationAction::Toggle).await.unwrap();
    a.until("tekst bez wielkiej litery", |a| {
        typed(a, w) == "dzień dobry."
    })
    .await;
    a.voice.dictation(DictationAction::Toggle).await.unwrap();
    let v = a
        .voice
        .dictation(DictationAction::RemoveProfile {
            app: "notepad.exe".into(),
        })
        .await
        .unwrap();
    assert!(v.dictation.profiles.is_empty());
}
