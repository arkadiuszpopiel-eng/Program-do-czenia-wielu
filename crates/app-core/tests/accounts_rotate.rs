//! Wymiana klucza i wyłączanie konta: działający klucz zostaje, dopóki nowy nie przejdzie testu,
//! a włączenie konta przywraca stan z ostatniego testu (przegląd systemu kluczy 2026-10-09).

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use app_core::AppCore;
use app_core::dto::{AccountAssignment, AccountState, Money};
use common::keys::*;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn rotating_keeps_the_working_key_until_the_new_one_is_proven() {
    let (base, seen) = provider_stub();
    let (core, _dir) = core().await;
    let account = core
        .accounts_add(input(GOOD, &base, "praca"))
        .await
        .unwrap();
    assert!(core.accounts_test(account.id.clone()).await.unwrap().ok);
    core.accounts_assign(
        account.id.clone(),
        AccountAssignment {
            task_classes: vec!["chat".into(), "code".into()],
            agents: vec!["beta".into()],
            voice_stt: false,
            voice_tts: false,
        },
    )
    .await
    .unwrap();
    core.accounts_set_limit(account.id.clone(), true, Money::pln(5_000))
        .await
        .unwrap();

    // Pomyłka przy wklejaniu: zły klucz nie zastępuje działającego.
    let report = core
        .accounts_rotate(account.id.clone(), secret(BAD))
        .await
        .unwrap();
    assert!(!report.ok);
    assert_eq!(report.reason.as_deref(), Some("invalid_key"));
    assert_eq!(
        state_of(&core, &account.id).await,
        (AccountState::Ok, "ok".to_owned()),
        "konto zostaje przy starym, działającym kluczu"
    );
    seen.lock().unwrap().clear();
    assert!(core.accounts_test(account.id.clone()).await.unwrap().ok);
    {
        // Test konta to dwa zapytania (połączenie i lista modeli) — oba tym samym kluczem.
        let keys = seen.lock().unwrap();
        assert!(
            !keys.is_empty() && keys.iter().all(|k| k == GOOD),
            "w magazynie został stary klucz: {keys:?}"
        );
    }

    // Poprawny nowy klucz (z cudzysłowami ze schowka): to samo konto, ustawienia zostają.
    let report = core
        .accounts_rotate(account.id.clone(), secret(&format!("  '{GOOD_2}' ")))
        .await
        .unwrap();
    assert!(report.ok, "{report:?}");
    let after = listed(&core, &account.id).await;
    assert_eq!(after.state, AccountState::Ok);
    assert_eq!(after.assignments.task_classes, vec!["chat", "code"]);
    assert_eq!(after.cost_limit.monthly, Money::pln(5_000));
    seen.lock().unwrap().clear();
    assert!(core.accounts_test(account.id.clone()).await.unwrap().ok);
    {
        // Test konta to dwa zapytania (połączenie i lista modeli) — oba tym samym kluczem.
        let keys = seen.lock().unwrap();
        assert!(
            !keys.is_empty() && keys.iter().all(|k| k == GOOD_2),
            "teraz używany jest nowy klucz: {keys:?}"
        );
    }

    assert!(
        core.accounts_rotate(account.id.clone(), secret("   "))
            .await
            .is_err()
    );
    // Klucz innego konta tego samego dostawcy pod tym samym adresem — odmowa z nazwą konta.
    let other = core
        .accounts_add(input(GOOD, &base, "drugie"))
        .await
        .unwrap();
    let taken = core
        .accounts_rotate(other.id, secret(GOOD_2))
        .await
        .unwrap_err();
    assert!(taken.to_string().contains("„praca”"), "{taken}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn enabling_an_account_restores_its_last_known_state() {
    let (base, _) = provider_stub();
    let (core, _dir) = core().await;
    let bad = core.accounts_add(input(BAD, &base, "zły")).await.unwrap();
    assert!(!core.accounts_test(bad.id.clone()).await.unwrap().ok);

    // „Włącz” na koncie, które nie jest wyłączone, niczego nie zmienia.
    let same = core
        .accounts_set_disabled(bad.id.clone(), false)
        .await
        .unwrap();
    assert_eq!(same.state, AccountState::Invalid);
    let off = core
        .accounts_set_disabled(bad.id.clone(), true)
        .await
        .unwrap();
    assert_eq!(off.state, AccountState::Disabled);
    assert!(off.key_stored, "wyłączenie nie usuwa klucza");
    // Odrzucony klucz nie staje się „działającym” przez wyłączenie i włączenie konta.
    let on = core
        .accounts_set_disabled(bad.id.clone(), false)
        .await
        .unwrap();
    assert_eq!(on.state, AccountState::Invalid);

    let good = core
        .accounts_add(input(GOOD, &base, "dobry"))
        .await
        .unwrap();
    assert!(core.accounts_test(good.id.clone()).await.unwrap().ok);
    core.accounts_set_disabled(good.id.clone(), true)
        .await
        .unwrap();
    // Test wyłączonego konta nie włącza go z powrotem.
    core.accounts_test(good.id.clone()).await.ok();
    assert_eq!(listed(&core, &good.id).await.state, AccountState::Disabled);
    let on = core
        .accounts_set_disabled(good.id.clone(), false)
        .await
        .unwrap();
    assert_eq!(on.state, AccountState::Ok);
}

/// Przegląd 2026-10-09: limit zapytań nie dowodzi, że kandydat jest dobry (429 potrafi przyjść przed
/// uwierzytelnieniem), więc nie zastępuje działającego klucza. Klucz rozpoznany przez dostawcę
/// (brak środków) i klucz, którego nie da się sprawdzić (adres bez listy modeli), są zapisywane —
/// inaczej takiego konta nie dałoby się naprawić bez usuwania.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn only_a_recognised_or_uncheckable_candidate_replaces_the_key() {
    let (base, seen) = provider_stub();
    let (core, _dir) = core().await;
    let account = core
        .accounts_add(input(GOOD, &base, "praca"))
        .await
        .unwrap();
    assert!(core.accounts_test(account.id.clone()).await.unwrap().ok);
    let stored_key = |core: &AppCore, id: String| {
        let seen = seen.clone();
        let core = core.clone();
        async move {
            seen.lock().unwrap().clear();
            let _ = core.accounts_test(id).await;
            seen.lock().unwrap().last().cloned().unwrap_or_default()
        }
    };

    let limited = core
        .accounts_rotate(account.id.clone(), secret(RATE_LIMITED))
        .await
        .unwrap();
    assert_eq!(limited.reason.as_deref(), Some("rate_limited"));
    assert_eq!(stored_key(&core, account.id.clone()).await, GOOD);
    assert_eq!(listed(&core, &account.id).await.state, AccountState::Ok);

    let broke = core
        .accounts_rotate(account.id.clone(), secret(NO_CREDIT))
        .await
        .unwrap();
    assert_eq!(broke.reason.as_deref(), Some("no_credit"));
    assert_eq!(stored_key(&core, account.id.clone()).await, NO_CREDIT);
    assert_eq!(
        state_of(&core, &account.id).await,
        (AccountState::Invalid, "no_credit".to_owned())
    );

    let unchecked = core
        .accounts_rotate(account.id.clone(), secret(NO_MODELS))
        .await
        .unwrap();
    assert_eq!(unchecked.reason.as_deref(), Some("unsupported"));
    assert_eq!(stored_key(&core, account.id.clone()).await, NO_MODELS);
}
