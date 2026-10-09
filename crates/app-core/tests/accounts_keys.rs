//! Klucze API od dodania do diagnozy (zgłoszenie właściciela 2026-10-09: każdy błąd 401 był
//! „kluczem odrzuconym”) oraz ustalenia przeglądu systemu kluczy z tego samego dnia. Cała ścieżka
//! idzie przez prawdziwy adapter dostawcy do lokalnego serwera, który odpowiada jak dostawca.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use app_core::dto::AccountState;
use common::keys::*;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn each_kind_of_refusal_gets_its_own_diagnosis() {
    let (base, _) = provider_stub();
    let (core, _dir) = core().await;
    let mut ids = Vec::new();
    // (klucz, czy działa, powód, kod HTTP, kod dostawcy, stan konta po teście)
    for (key, ok, reason, status, code, state) in [
        (
            BAD,
            false,
            "invalid_key",
            Some(401),
            Some("invalid_api_key"),
            AccountState::Invalid,
        ),
        (
            RESTRICTED,
            true,
            "restricted",
            Some(401),
            None,
            AccountState::Ok,
        ),
        (
            NO_CREDIT,
            false,
            "no_credit",
            Some(429),
            Some("insufficient_quota"),
            AccountState::Invalid,
        ),
        (
            NO_BALANCE,
            false,
            "no_credit",
            Some(402),
            None,
            AccountState::Invalid,
        ),
        (
            REGION,
            false,
            "refused",
            Some(403),
            Some("unsupported_country_region_territory"),
            AccountState::Invalid,
        ),
    ] {
        let account = core.accounts_add(input(key, &base, reason)).await.unwrap();
        let report = core.accounts_test(account.id.clone()).await.unwrap();
        assert_eq!(report.ok, ok, "{key}: {report:?}");
        assert_eq!(report.reason.as_deref(), Some(reason), "{key}");
        assert_eq!(report.status, status, "{key}");
        if code.is_some() {
            assert_eq!(report.provider_code.as_deref(), code, "{key}");
        }
        assert_eq!(
            report.error.is_none(),
            ok,
            "{key}: opis błędu tylko przy porażce"
        );
        assert_eq!(
            state_of(&core, &account.id).await,
            (state, reason.to_owned()),
            "{key}: lista kont pamięta powód"
        );
        ids.push(account.id);
    }

    // Dostawca odsyła klucz zamaskowany — jego początek i koniec nie trafiają do interfejsu ani na dysk.
    let bad = serde_json::to_string(&listed(&core, &ids[0]).await).unwrap();
    assert!(bad.contains("Incorrect API key provided: [•••]"), "{bad}");
    assert!(!bad.contains("wxyz") && !bad.contains("sk-proj"), "{bad}");

    let good = core
        .accounts_add(input(GOOD, &base, "dobry"))
        .await
        .unwrap();
    let report = core.accounts_test(good.id.clone()).await.unwrap();
    assert!(report.ok);
    assert_eq!((report.reason, report.models.len()), (None, 1));

    // Adres bez listy modeli: klucza nie da się sprawdzić z góry, ale konto nie staje się „błędem”.
    let custom = core
        .accounts_add(input(NO_MODELS, &base, "własny serwer"))
        .await
        .unwrap();
    let report = core.accounts_test(custom.id.clone()).await.unwrap();
    assert_eq!(report.reason.as_deref(), Some("unsupported"));
    assert_eq!(listed(&core, &custom.id).await.state, AccountState::Ok);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn restricted_key_becomes_usable_once_a_model_is_named() {
    let (base, _) = provider_stub();
    let (core, _dir) = core().await;
    let account = core
        .accounts_add(input(RESTRICTED, &base, "ograniczony"))
        .await
        .unwrap();
    assert!(core.accounts_test(account.id.clone()).await.unwrap().ok);
    assert!(listed(&core, &account.id).await.models.is_empty());
    assert!(
        core.accounts_set_model(account.id.clone(), "  ".into())
            .await
            .is_err()
    );

    let named = core
        .accounts_set_model(account.id.clone(), " model-recznie ".into())
        .await
        .unwrap();
    assert_eq!(named.models.len(), 1);
    assert_eq!(named.models[0].id, "model-recznie");
    // Kolejny test (nadal bez prawa do listy modeli) nie kasuje modelu wpisanego ręcznie.
    assert!(core.accounts_test(account.id.clone()).await.unwrap().ok);
    assert_eq!(
        listed(&core, &account.id).await.models[0].id,
        "model-recznie"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn pasted_extras_are_removed_prose_is_refused_and_duplicates_are_caught() {
    let (base, seen) = provider_stub();
    let (other_base, _) = provider_stub();
    let (core, _dir) = core().await;

    // Tak wygląda klucz skopiowany z pliku .env razem z końcem wiersza.
    let pasted = format!("OPENAI_API_KEY=\"{GOOD}\"\r\n");
    let account = core
        .accounts_add(input(&pasted, &base, "z pliku env"))
        .await
        .unwrap();
    assert!(core.accounts_test(account.id.clone()).await.unwrap().ok);
    assert!(
        seen.lock().unwrap().iter().all(|key| key == GOOD),
        "do dostawcy ma dotrzeć sam klucz: {:?}",
        seen.lock().unwrap()
    );

    // Zdanie z kluczem w środku nie jest sklejane w „klucz” — czytelna odmowa zamiast 401 od dostawcy.
    let prose = core
        .accounts_add(input(&format!("Mój klucz: {GOOD_2}"), &base, "zdanie"))
        .await
        .unwrap_err();
    assert!(
        prose.to_string().contains("nie wygląda na klucz API"),
        "{prose}"
    );

    let twice = core
        .accounts_add(input(&format!("Bearer {GOOD}"), &base, "drugi raz"))
        .await
        .unwrap_err();
    let message = twice.to_string();
    assert!(
        message.contains("już dodany") && message.contains("z pliku env"),
        "{message}"
    );
    // Ten sam klucz pod innym adresem to inne konto (dwa własne serwery z tym samym kluczem).
    core.accounts_add(input(GOOD, &other_base, "inny serwer"))
        .await
        .unwrap();
    assert_eq!(core.accounts_list().await.unwrap().len(), 2);
}
