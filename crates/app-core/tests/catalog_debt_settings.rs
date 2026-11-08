//! Dług katalogu funkcji (docs/plan/45-testy-ai.md): cennik modeli i ustawienia dla firm na
//! rdzeniu — podgląd zmian cen niczego nie zapisuje, zapis trafia do pliku użytkownika
//! w katalogu konfiguracji profilu testowego; ustawienia firmowe konta są sprawdzane przez hub,
//! ogłaszane zdarzeniem i odrzucane z polem formularza. Bez sieci (klucze testowe nie są
//! sprawdzane u dostawcy).

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use app_core::ErrorCode;
use app_core::dto::{Account, AddAccountInput, AlfaEvent, AzureDeployment, EnterpriseSettings};
use common::{Harness, harness, until};
use serde_json::json;

const UPDATE: &str = "checked = \"2026-11-02\"\n\
    [[price]]\nprovider = \"openai\"\nmodel = \"gpt-test-luna\"\ninput = 0.08\noutput = 0.4\n\
    source = \"https://example.invalid/cennik\"\n";

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn pricing_get_shows_builtin_prices_and_user_file_location() {
    let h = harness().await;
    let view = h.core.pricing_get().await.unwrap();
    assert!(!view.rows.is_empty(), "ceny wbudowane w wydanie");
    assert!(view.rows.iter().all(|r| !r.user), "bez pliku użytkownika");
    assert!(view.changes.is_empty() && !view.saved);
    let expected = h.dir.path().join("config").join("cennik.toml");
    assert_eq!(view.file, expected.display().to_string());
    assert!(!expected.exists());
    assert!(
        view.rows
            .iter()
            .all(|r| r.input >= 0.0 && r.output >= 0.0 && !r.provider.is_empty())
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn pricing_apply_previews_then_saves_for_later_reads() {
    let h = harness().await;
    let file = h.dir.path().join("config").join("cennik.toml");
    let before = h.core.pricing_get().await.unwrap();

    let preview = h.core.pricing_apply(UPDATE.into(), false).await.unwrap();
    assert!(!preview.saved);
    assert_eq!(preview.changes.len(), 1);
    let change = &preview.changes[0];
    assert_eq!(change.after.model, "gpt-test-luna");
    assert!(change.before.is_none(), "nowy model");
    assert!(!file.exists(), "podgląd niczego nie zapisuje");
    assert_eq!(h.core.pricing_get().await.unwrap(), before);

    let saved = h.core.pricing_apply(UPDATE.into(), true).await.unwrap();
    assert!(saved.saved && file.exists());
    assert_eq!(saved.checked, "2026-11-02");
    let read = h.core.pricing_get().await.unwrap();
    let luna = read
        .rows
        .iter()
        .find(|r| r.provider == "openai" && r.model == "gpt-test-luna")
        .unwrap();
    assert!(luna.user && (luna.input - 0.08).abs() < 1e-9 && (luna.output - 0.4).abs() < 1e-9);
    assert_eq!(read.rows.len(), before.rows.len() + 1);

    // Ta sama aktualizacja ponownie — nie ma już zmian do pokazania.
    let again = h.core.pricing_apply(UPDATE.into(), false).await.unwrap();
    assert!(again.changes.is_empty(), "{:?}", again.changes);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn pricing_apply_refuses_text_without_prices() {
    let h = harness().await;
    let file = h.dir.path().join("config").join("cennik.toml");
    for text in [
        "",
        "checked = \"2026-11-02\"",
        "[[price]]\nprovider = \"a\"",
        "to nie jest TOML [[",
    ] {
        for save in [false, true] {
            let err = h.core.pricing_apply(text.into(), save).await.unwrap_err();
            assert_eq!(err.code, ErrorCode::InvalidInput, "{text:?}/{save}");
        }
    }
    assert!(!file.exists(), "błędny tekst niczego nie zapisuje");
}

fn new_account(provider: &str, base_url: Option<&str>) -> AddAccountInput {
    serde_json::from_value(json!({
        "provider_id": provider,
        "label": "Firma",
        "secret": format!("sk-test-{provider}-XXXXXXXXXXXXXXXX"),
        "base_url": base_url,
    }))
    .unwrap()
}

fn openai(organization: Option<&str>, project: Option<&str>) -> EnterpriseSettings {
    EnterpriseSettings {
        organization: organization.map(str::to_owned),
        project: project.map(str::to_owned),
        ..EnterpriseSettings::default()
    }
}

async fn listed(h: &Harness, id: &str) -> Account {
    let all = h.core.accounts_list().await.unwrap();
    all.into_iter().find(|a| a.id == id).unwrap()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn accounts_set_enterprise_saves_openai_organization_and_project() {
    let mut h = harness().await;
    let account = h
        .core
        .accounts_add(new_account("openai", None))
        .await
        .unwrap();
    let changed = h
        .core
        .accounts_set_enterprise(
            account.id.clone(),
            openai(Some(" org-acme1234 "), Some("proj_budget_a")),
        )
        .await
        .unwrap();
    let expected = openai(Some("org-acme1234"), Some("proj_budget_a"));
    assert_eq!(changed.enterprise, expected, "oczyszczone");
    assert_eq!(listed(&h, &account.id).await.enterprise, expected);
    let id = account.id.clone();
    until(&mut h.rx, |e| {
        matches!(e, AlfaEvent::AccountChanged { account } if account.id == id && account.enterprise.organization.is_some())
    })
    .await;

    // Odmowa (zły wzór) — błąd na polu formularza, konto bez zmian.
    for bad in [openai(Some("acme"), None), openai(None, Some("budget"))] {
        let err = h
            .core
            .accounts_set_enterprise(account.id.clone(), bad)
            .await
            .unwrap_err();
        assert_eq!(err.code, ErrorCode::InvalidInput, "{err:?}");
        assert_eq!(err.details.text("field"), Some("enterprise"));
        assert!(err.message.ends_with('.'), "zdanie: {}", err.message);
    }
    assert_eq!(listed(&h, &account.id).await.enterprise, expected);

    // Puste pola czyszczą ustawienia firmowe.
    let cleared = h
        .core
        .accounts_set_enterprise(account.id.clone(), openai(Some(""), Some("  ")))
        .await
        .unwrap();
    assert_eq!(cleared.enterprise, EnterpriseSettings::default());
}

// Azure OpenAI czeka w katalogu na właściciela (`providers-catalog/czeka-na-wlasciciela`), więc
// rdzeń nie założy takiego konta; reguły Azure sprawdzają testy kontraktowe huba.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn accounts_set_enterprise_keeps_one_key_per_openai_project() {
    let h = harness().await;
    let mut first = new_account("openai", None);
    first.label = "Budżet A".into();
    first.enterprise = openai(None, Some("proj_budget_a"));
    let a = h.core.accounts_add(first).await.unwrap();
    let mut second = new_account("openai", None);
    second.enterprise = openai(None, Some("proj_budget_b"));
    let b = h.core.accounts_add(second).await.unwrap();

    // Ten sam klucz w tym samym projekcie to duplikat — odmowa z nazwą konta.
    let err = h
        .core
        .accounts_set_enterprise(b.id.clone(), openai(None, Some(" proj_budget_a ")))
        .await
        .unwrap_err();
    assert_eq!(err.code, ErrorCode::InvalidInput);
    assert!(err.message.contains("Budżet A"), "{}", err.message);
    assert_eq!(
        listed(&h, &b.id).await.enterprise,
        openai(None, Some("proj_budget_b"))
    );
    // Wdrożenia Azure nie dotyczą konta OpenAI.
    let azure = EnterpriseSettings {
        api_version: Some("2025-04-01-preview".into()),
        deployments: vec![AzureDeployment {
            name: "prod-4o".into(),
            model: None,
        }],
        ..EnterpriseSettings::default()
    };
    let err = h
        .core
        .accounts_set_enterprise(a.id.clone(), azure)
        .await
        .unwrap_err();
    assert_eq!(err.code, ErrorCode::InvalidInput);
    // Zapis tego samego projektu na tym samym koncie nie jest duplikatem; inny projekt — wolno.
    let same = h
        .core
        .accounts_set_enterprise(
            a.id.clone(),
            openai(Some("org-acme1234"), Some("proj_budget_a")),
        )
        .await
        .unwrap();
    assert_eq!(
        same.enterprise.organization.as_deref(),
        Some("org-acme1234")
    );
    let moved = h
        .core
        .accounts_set_enterprise(b.id.clone(), openai(None, Some("proj_budget_c")))
        .await
        .unwrap();
    assert_eq!(moved.enterprise.project.as_deref(), Some("proj_budget_c"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn accounts_set_enterprise_refuses_other_providers_and_unknown_accounts() {
    let h = harness().await;
    let other = h
        .core
        .accounts_add(new_account(
            "custom-openai-compatible",
            Some("http://127.0.0.1:9/v1"),
        ))
        .await
        .unwrap();
    let err = h
        .core
        .accounts_set_enterprise(other.id.clone(), openai(Some("org-acme1234"), None))
        .await
        .unwrap_err();
    assert_eq!(err.code, ErrorCode::InvalidInput);
    assert!(err.message.contains("OpenAI"), "{}", err.message);
    assert!(
        listed(&h, &other.id)
            .await
            .enterprise
            .organization
            .is_none()
    );

    let missing = h
        .core
        .accounts_set_enterprise("acc-nie-ma".into(), EnterpriseSettings::default())
        .await
        .unwrap_err();
    assert_eq!(missing.code, ErrorCode::NotFound);
    let malformed = h
        .core
        .accounts_set_enterprise(String::new(), EnterpriseSettings::default())
        .await
        .unwrap_err();
    assert_eq!(malformed.code, ErrorCode::InvalidInput);
}
