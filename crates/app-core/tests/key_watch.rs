//! Okresowe sprawdzanie kluczy API w tle i data zapisu klucza (plan v6, zadanie C2.14). Cała
//! ścieżka idzie przez prawdziwy adapter do lokalnego serwera udającego dostawcę; serwer ma
//! przełącznik „klucz przestał działać”.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use app_core::dto::{AccountState, AlfaEvent, ToastKind};
use chrono::{Duration, Utc};
use common::keys::{GOOD, GOOD_2, core, input, listed, reply, secret};

/// Dostawca z przełącznikiem: po `revoke` każdy klucz jest odrzucany jak nieważny.
struct Stub {
    base: String,
    revoked: Arc<AtomicBool>,
    calls: Arc<AtomicUsize>,
}

fn stub() -> Stub {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}/v1", listener.local_addr().unwrap());
    let revoked = Arc::new(AtomicBool::new(false));
    let calls = Arc::new(AtomicUsize::new(0));
    let (flag, count) = (revoked.clone(), calls.clone());
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };
            let mut buffer = Vec::new();
            let mut chunk = [0u8; 1024];
            while !buffer.windows(4).any(|w| w == b"\r\n\r\n") {
                match stream.read(&mut chunk) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => buffer.extend_from_slice(&chunk[..n]),
                }
            }
            count.fetch_add(1, Ordering::SeqCst);
            let key = if flag.load(Ordering::SeqCst) {
                "bad"
            } else {
                "dobry"
            };
            let (status, body) = reply(key);
            let response = format!(
                "HTTP/1.1 {status}\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                body.len()
            );
            let _ = stream.write_all(response.as_bytes());
        }
    });
    Stub {
        base,
        revoked,
        calls,
    }
}

async fn set_interval(core: &app_core::AppCore, hours: u64) {
    core.settings_set(
        "accounts.check_interval_hours".into(),
        serde_json::from_value(serde_json::json!(hours)).unwrap(),
    )
    .await
    .unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn due_accounts_are_checked_and_a_broken_key_is_announced_once() {
    let stub = stub();
    let (core, _dir) = core().await;
    let account = core
        .accounts_add(input(GOOD, &stub.base, "praca"))
        .await
        .unwrap();
    assert!(core.accounts_test(account.id.clone()).await.unwrap().ok);
    let mut rx = core.subscribe_events();
    let now = Utc::now();

    // Świeżo sprawdzone konto nie jest do sprawdzenia; po upływie odstępu — jest.
    let before = stub.calls.load(Ordering::SeqCst);
    assert!(core.check_keys_due(now).await.checked.is_empty());
    assert_eq!(
        stub.calls.load(Ordering::SeqCst),
        before,
        "żadnego zapytania"
    );
    let round = core.check_keys_due(now + Duration::hours(25)).await;
    assert_eq!(round.checked, vec![account.id.clone()]);
    assert!(round.broken.is_empty());
    assert_eq!(listed(&core, &account.id).await.state, AccountState::Ok);

    // Klucz przestaje działać u dostawcy: konto zmienia stan i jest jeden komunikat.
    stub.revoked.store(true, Ordering::SeqCst);
    let round = core.check_keys_due(now + Duration::hours(50)).await;
    assert_eq!(round.broken, vec![account.id.clone()]);
    let after = listed(&core, &account.id).await;
    assert_eq!(after.state, AccountState::Invalid);
    assert_eq!(after.last_test.unwrap().reason, "invalid_key");
    // Kolejny przebieg sprawdza konto dalej (może wrócić do życia), ale już nie ogłasza.
    let round = core.check_keys_due(now + Duration::hours(80)).await;
    assert_eq!(round.checked.len(), 1);
    assert!(round.broken.is_empty());

    tokio::time::sleep(std::time::Duration::from_millis(80)).await;
    let mut warnings = 0;
    while let Ok(batch) = rx.try_recv() {
        for event in batch.iter() {
            if let AlfaEvent::Toast { kind, message } = event {
                assert_eq!(*kind, ToastKind::Warning);
                assert!(message.pl.contains("praca"), "{}", message.pl);
                assert!(!message.pl.contains(GOOD), "komunikat nie niesie klucza");
                warnings += 1;
            }
        }
    }
    assert_eq!(warnings, 1);

    // Klucz znów działa: konto wraca do „ok” bez udziału użytkownika.
    stub.revoked.store(false, Ordering::SeqCst);
    core.check_keys_due(now + Duration::hours(120)).await;
    assert_eq!(listed(&core, &account.id).await.state, AccountState::Ok);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn switched_off_checks_and_disabled_accounts_make_no_requests() {
    let stub = stub();
    let (core, _dir) = core().await;
    let account = core
        .accounts_add(input(GOOD, &stub.base, "praca"))
        .await
        .unwrap();
    assert!(core.accounts_test(account.id.clone()).await.unwrap().ok);
    let later = Utc::now() + Duration::days(30);
    let calls = || stub.calls.load(Ordering::SeqCst);

    // Odstęp 0 = sprawdzanie wyłączone.
    set_interval(&core, 0).await;
    let before = calls();
    assert!(core.check_keys_due(later).await.checked.is_empty());
    assert_eq!(calls(), before);

    // Konto wyłączone jest pomijane także przy włączonym sprawdzaniu.
    set_interval(&core, 6).await;
    core.accounts_set_disabled(account.id.clone(), true)
        .await
        .unwrap();
    assert!(core.check_keys_due(later).await.checked.is_empty());
    assert_eq!(calls(), before);

    core.accounts_set_disabled(account.id.clone(), false)
        .await
        .unwrap();
    assert_eq!(core.check_keys_due(later).await.checked.len(), 1);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn key_date_is_set_on_add_and_moves_on_rotation() {
    let stub = stub();
    let (core, _dir) = core().await;
    let started = Utc::now() - Duration::seconds(2);
    let account = core
        .accounts_add(input(GOOD, &stub.base, "praca"))
        .await
        .unwrap();
    let parse = |text: &str| {
        chrono::DateTime::parse_from_rfc3339(text)
            .unwrap()
            .with_timezone(&Utc)
    };
    let saved = parse(account.key_saved_at.as_deref().expect("data zapisu klucza"));
    assert!(saved >= started);

    tokio::time::sleep(std::time::Duration::from_millis(1100)).await;
    let report = core
        .accounts_rotate(account.id.clone(), secret(GOOD_2))
        .await
        .unwrap();
    assert!(report.ok);
    let rotated = parse(
        listed(&core, &account.id)
            .await
            .key_saved_at
            .as_deref()
            .unwrap(),
    );
    assert!(
        rotated > saved,
        "wymiana klucza przesuwa datę: {saved} → {rotated}"
    );
}
