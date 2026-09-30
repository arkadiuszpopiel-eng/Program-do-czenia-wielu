//! Przypadki testów kontraktowych huba kont.

use super::{KEY_INVALID, KEY_NET, KEY_OK, KEY_OK_2, KEY_RATE, MODEL_A, MODEL_B, SpyEnv};
use crate::{
    AccountErrorKind, AccountSource, AccountState, AccountsError, AccountsHub, Assignments,
    CostLimit, EnvImportOutcome, ModelId, ModelPrice, NewAccount, PriceTable, ProviderId,
    SecretString, TaskClass, TestOutcome, VoiceRole, Wizard, WizardStep,
};

fn pid(s: &str) -> ProviderId {
    ProviderId::new(s).unwrap_or_else(|e| panic!("{e}"))
}

fn new_account(provider: &str, key: &str) -> NewAccount {
    NewAccount {
        provider: pid(provider),
        label: format!("konto {provider}"),
        secret: SecretString::from(key),
        base_url: None,
        assignments: Assignments::default(),
        cost_limit: None,
        source: AccountSource::Manual,
    }
}

fn ok<T, E: std::fmt::Display>(r: Result<T, E>) -> T {
    r.unwrap_or_else(|e| panic!("{e}"))
}

/// Bez kont każdy dostawca jest „nieskonfigurowany”.
pub async fn unconfigured_without_accounts<H: AccountsHub>(hub: &H) {
    let catalog = hub.catalog();
    assert_eq!(catalog.len(), 4);
    assert!(hub.accounts().is_empty());
    for entry in catalog {
        assert_eq!(hub.provider_state(&entry.id), AccountState::Unconfigured);
    }
    assert!(hub.provider(&pid("nope")).is_none());
}

/// Dodanie → test → odczyt klucza przez adapter → usunięcie (0 dostępów po usunięciu).
pub async fn add_test_remove<H: AccountsHub>(hub: &H) {
    let id = ok(hub.add_account(new_account("acme", KEY_OK)).await);
    let acc = hub.account(&id).unwrap_or_else(|| panic!("brak konta"));
    assert_eq!(acc.state, AccountState::Active);
    assert_eq!(hub.provider_state(&pid("acme")), AccountState::Active);
    let summary = ok(hub.test_account(&id).await);
    assert_eq!(summary.outcome, TestOutcome::Ok);
    assert_eq!(summary.models_found, Some(2));
    let acc = hub.account(&id).unwrap_or_else(|| panic!("brak konta"));
    assert_eq!(acc.last_test, Some(summary));
    assert_eq!(acc.models.len(), 2);
    let key = ok(hub.resolve_secret(&id, "providers-api"));
    assert_eq!(key.expose_secret(), KEY_OK);
    assert!(matches!(
        hub.resolve_secret(&id, "router"),
        Err(AccountsError::NotPermitted(_))
    ));
    ok(hub.remove(&id).await);
    assert!(hub.account(&id).is_none());
    assert_eq!(hub.provider_state(&pid("acme")), AccountState::Unconfigured);
    assert!(matches!(
        hub.resolve_secret(&id, "providers-api"),
        Err(AccountsError::UnknownAccount(_))
    ));
    assert!(matches!(
        hub.remove(&id).await,
        Err(AccountsError::UnknownAccount(_))
    ));
}

/// Zły klucz → stan błędu; rotacja na dobry → aktywny po teście.
pub async fn invalid_key_then_rotate<H: AccountsHub>(hub: &H) {
    let id = ok(hub.add_account(new_account("acme", KEY_INVALID)).await);
    let summary = ok(hub.test_account(&id).await);
    assert_eq!(summary.outcome, TestOutcome::InvalidKey);
    let err = AccountState::Error {
        kind: AccountErrorKind::InvalidKey,
    };
    assert_eq!(hub.account(&id).map(|a| a.state), Some(err.clone()));
    assert_eq!(hub.provider_state(&pid("acme")), err);
    ok(hub.rotate(&id, SecretString::from(KEY_OK_2)).await);
    assert_eq!(
        ok(hub.resolve_secret(&id, "providers-api")).expose_secret(),
        KEY_OK_2
    );
    assert_eq!(ok(hub.test_account(&id).await).outcome, TestOutcome::Ok);
    assert_eq!(hub.provider_state(&pid("acme")), AccountState::Active);
    let rate = ok(hub.add_account(new_account("acme", KEY_RATE)).await);
    assert_eq!(
        ok(hub.test_account(&rate).await).outcome,
        TestOutcome::RateLimited
    );
    assert_eq!(
        hub.account(&rate).map(|a| a.state),
        Some(AccountState::Active)
    );
    assert!(matches!(
        hub.rotate(&id, SecretString::from("")).await,
        Err(AccountsError::InvalidInput(_))
    ));
}

/// Komunikat błędu sieci z kluczem w treści jest redagowany.
pub async fn network_error_is_redacted<H: AccountsHub>(hub: &H) {
    let id = ok(hub.add_account(new_account("acme", KEY_NET)).await);
    let summary = ok(hub.test_account(&id).await);
    assert!(matches!(summary.outcome, TestOutcome::Network { .. }));
    let acc = hub.account(&id).unwrap_or_else(|| panic!("brak konta"));
    assert_eq!(
        acc.state,
        AccountState::Error {
            kind: AccountErrorKind::Network
        }
    );
    let dump = format!("{acc:?} {summary:?}");
    assert!(!dump.contains(KEY_NET), "{dump}");
}

/// Wyłączenie: stan „wyłączony”, test go nie zmienia, klucz niedostępny; włączenie przywraca.
pub async fn disable_and_enable<H: AccountsHub>(hub: &H) {
    let id = ok(hub.add_account(new_account("acme", KEY_OK)).await);
    ok(hub.set_disabled(&id, true).await);
    assert_eq!(hub.provider_state(&pid("acme")), AccountState::Disabled);
    ok(hub.test_account(&id).await);
    assert_eq!(
        hub.account(&id).map(|a| a.state),
        Some(AccountState::Disabled)
    );
    assert!(matches!(
        hub.resolve_secret(&id, "providers-api"),
        Err(AccountsError::AccountDisabled(_))
    ));
    ok(hub.set_disabled(&id, false).await);
    assert_eq!(
        hub.account(&id).map(|a| a.state),
        Some(AccountState::Active)
    );
}

/// Dostawca zabroniony, zły klucz, brak/zły endpoint, logowanie CLI — odrzucone.
pub async fn rejects_forbidden_and_invalid_input<H: AccountsHub>(hub: &H) {
    assert!(matches!(
        hub.add_account(new_account("banned", KEY_OK)).await,
        Err(AccountsError::ProviderForbidden(_))
    ));
    assert!(matches!(
        hub.add_account(new_account("acme", "sk ok")).await,
        Err(AccountsError::InvalidInput(_))
    ));
    assert!(matches!(
        hub.add_account(new_account("nope", KEY_OK)).await,
        Err(AccountsError::UnknownProvider(_))
    ));
    assert!(matches!(
        hub.add_account(new_account("clionly", KEY_OK)).await,
        Err(AccountsError::InvalidInput(_))
    ));
    assert!(matches!(
        hub.add_account(new_account("selfhost", KEY_OK)).await,
        Err(AccountsError::InvalidInput(_))
    ));
    let mut bad_url = new_account("selfhost", KEY_OK);
    bad_url.base_url = Some("http://evil.example/v1".into());
    assert!(matches!(
        hub.add_account(bad_url).await,
        Err(AccountsError::InvalidInput(_))
    ));
    let mut good = new_account("selfhost", KEY_OK);
    good.base_url = Some("http://localhost:11434/v1".into());
    let id = ok(hub.add_account(good).await);
    assert_eq!(
        hub.account(&id).and_then(|a| a.base_url).as_deref(),
        Some("http://localhost:11434/v1")
    );
    assert!(hub.accounts().len() == 1);
}

/// Kreator: dostawca → klucz → test → modele → przypisania → limit → zapis.
pub async fn wizard_happy_path<H: AccountsHub>(hub: &H) {
    let mut w = Wizard::new();
    let acme = hub
        .provider(&pid("acme"))
        .unwrap_or_else(|| panic!("brak acme"));
    ok(w.choose_provider(acme));
    ok(w.enter_key(
        SecretString::from_input(&format!(" {KEY_OK}\n")),
        "Moje Acme",
        None,
    ));
    ok(hub.wizard_test(&mut w).await);
    assert_eq!(w.step(), WizardStep::DiscoverModels);
    ok(hub.wizard_discover(&mut w).await);
    assert_eq!(w.step(), WizardStep::Assign);
    let names: Vec<&str> = w.models().iter().map(|m| m.id.as_str()).collect();
    assert_eq!(names, vec![MODEL_A, MODEL_B]);
    let assignments = Assignments {
        task_classes: [TaskClass::Conversation, TaskClass::Code].into(),
        agents: ["alfa".to_owned()].into(),
        voice: [VoiceRole::Tts].into(),
    };
    ok(w.assign(assignments.clone()));
    let limit = CostLimit {
        enabled: true,
        monthly_limit_grosze: 5_000,
    };
    ok(w.set_cost_limit(Some(limit)));
    let id = ok(hub.wizard_finish(w).await);
    let acc = hub.account(&id).unwrap_or_else(|| panic!("brak konta"));
    assert_eq!(acc.label, "Moje Acme");
    assert_eq!(acc.assignments, assignments);
    assert_eq!(acc.cost_limit, Some(limit));
    assert_eq!(acc.source, AccountSource::Wizard);
    assert_eq!(acc.models.len(), 2);
    assert_eq!(acc.last_test.map(|t| t.outcome), Some(TestOutcome::Ok));
    assert_eq!(
        ok(hub.resolve_secret(&id, "providers-api")).expose_secret(),
        KEY_OK
    );
}

/// Kreator: zły klucz wraca do kroku klucza; zapis przed potwierdzeniem jest błędem.
pub async fn wizard_bad_key_returns_to_key_step<H: AccountsHub>(hub: &H) {
    let mut w = Wizard::new();
    let acme = hub
        .provider(&pid("acme"))
        .unwrap_or_else(|| panic!("brak acme"));
    ok(w.choose_provider(acme));
    ok(w.enter_key(SecretString::from(KEY_INVALID), "", None));
    ok(hub.wizard_test(&mut w).await);
    assert_eq!(w.step(), WizardStep::EnterKey);
    assert_eq!(
        w.last_test().map(|t| t.outcome.clone()),
        Some(TestOutcome::InvalidKey)
    );
    assert!(hub.wizard_discover(&mut w).await.is_err());
    assert!(matches!(
        hub.wizard_finish(w).await,
        Err(AccountsError::Wizard(_))
    ));
    assert!(hub.accounts().is_empty());
}

/// Import ze środowiska czyta tylko zmienne z katalogu; powtórny import nie dubluje kont.
pub async fn env_import_reads_only_catalog_vars<H: AccountsHub>(hub: &H) {
    let env = SpyEnv::new(&[
        ("ACME_API_KEY", KEY_OK),
        ("BANNED_API_KEY", KEY_OK),
        ("HOME_SECRET", "x"),
    ]);
    let first = ok(hub.import_from_env(&env).await);
    assert_eq!(first.len(), 1, "{first:?}");
    assert_eq!(first[0].var, "ACME_API_KEY");
    let EnvImportOutcome::Imported { account } = first[0].outcome.clone() else {
        panic!("{first:?}");
    };
    assert_eq!(
        hub.account(&account).map(|a| a.source),
        Some(AccountSource::Env {
            var: "ACME_API_KEY".into()
        })
    );
    let second = ok(hub.import_from_env(&env).await);
    assert_eq!(
        second[0].outcome,
        EnvImportOutcome::AlreadyPresent { account }
    );
    assert_eq!(env.asked(), ["ACME_API_KEY".to_owned()].into());
    let empty = ok(hub.import_from_env(&SpyEnv::new(&[])).await);
    assert_eq!(empty[0].outcome, EnvImportOutcome::NotSet);
    assert_eq!(hub.accounts().len(), 1);
}

/// Cennik z konfiguracji i modele wykryte w kontach są widoczne w katalogu.
pub async fn pricing_and_models_in_catalog<H: AccountsHub>(hub: &H) {
    let mut table = PriceTable::default();
    let model = ModelId::new(MODEL_A).unwrap_or_else(|e| panic!("{e}"));
    table.0.insert(
        model.clone(),
        ModelPrice {
            input_micro_usd_per_mtok: 3_000_000,
            output_micro_usd_per_mtok: 15_000_000,
            ..ModelPrice::default()
        },
    );
    ok(hub.set_pricing(&pid("acme"), table.clone()));
    assert!(hub.set_pricing(&pid("nope"), table.clone()).is_err());
    let id = ok(hub.add_account(new_account("acme", KEY_OK)).await);
    ok(hub.test_account(&id).await);
    let acme = hub
        .provider(&pid("acme"))
        .unwrap_or_else(|| panic!("brak acme"));
    assert_eq!(acme.pricing, table);
    assert!(acme.pricing.price_for(&model).is_some());
    assert_eq!(acme.models.len(), 2);
}

/// Wartość klucza nie występuje w `Debug` kont, katalogu ani kreatora.
pub async fn secrets_never_in_debug<H: AccountsHub>(hub: &H) {
    let id = ok(hub.add_account(new_account("acme", KEY_OK)).await);
    ok(hub.test_account(&id).await);
    let mut w = Wizard::new();
    let acme = hub
        .provider(&pid("acme"))
        .unwrap_or_else(|| panic!("brak acme"));
    ok(w.choose_provider(acme));
    ok(w.enter_key(SecretString::from(KEY_OK_2), "x", None));
    let dump = format!("{:?} {:?} {:?}", hub.accounts(), hub.catalog(), w);
    assert!(!dump.contains(KEY_OK) && !dump.contains(KEY_OK_2), "{dump}");
}
