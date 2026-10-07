//! Property-based: żadna sekwencja operacji kreatora nie kończy się zapisem konta bez udanego
//! testu aktualnie wpisanego klucza; klucz nigdy nie pojawia się w `Debug`.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use accounts_hub_contract::contract_tests::fixture_catalog;
use accounts_hub_contract::{
    Assignments, ConnectionReport, CostLimit, ModelListError, SecretString, TestOutcome, Wizard,
    WizardError, WizardStep,
};
use proptest::prelude::*;

const KEYS: [&str; 4] = ["sk-ok-prop-1111", "sk-bad-prop-2222", "", "a b"];

#[derive(Debug, Clone)]
enum Op {
    Choose(usize),
    EnterKey(usize, bool),
    Test(usize),
    Models(bool),
    Assign,
    Limit(bool),
    Back,
    Finish,
}

fn op() -> impl Strategy<Value = Op> {
    prop_oneof![
        (0usize..4).prop_map(Op::Choose),
        (0usize..4, any::<bool>()).prop_map(|(k, u)| Op::EnterKey(k, u)),
        (0usize..4).prop_map(Op::Test),
        any::<bool>().prop_map(Op::Models),
        Just(Op::Assign),
        any::<bool>().prop_map(Op::Limit),
        Just(Op::Back),
        Just(Op::Finish),
    ]
}

fn outcome(i: usize) -> TestOutcome {
    match i {
        0 => TestOutcome::Ok,
        1 => TestOutcome::InvalidKey,
        2 => TestOutcome::RateLimited,
        _ => TestOutcome::Timeout,
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    #[test]
    fn finish_requires_passing_test_of_current_key(ops in prop::collection::vec(op(), 0..40)) {
        let catalog = fixture_catalog();
        let mut w = Wizard::new();
        let mut tested_ok = false;
        let mut current_key: Option<&str> = None;
        for op in ops {
            match op {
                Op::Choose(i) => {
                    if w.choose_provider(catalog[i].clone()).is_ok() {
                        tested_ok = false;
                    }
                }
                Op::EnterKey(k, url) => {
                    let base = url.then(|| "https://custom.example/v1".to_owned());
                    if w.enter_key(SecretString::from(KEYS[k]), "etykieta", base).is_ok() {
                        tested_ok = false;
                        current_key = Some(KEYS[k]);
                    }
                }
                Op::Test(i) => {
                    let o = outcome(i);
                    let works = o.key_works();
                    if w.record_test(ConnectionReport { outcome: o, latency_ms: Some(5) }).is_ok() {
                        tested_ok = works;
                    }
                }
                Op::Models(good) => {
                    let r = if good { Ok(vec![]) } else { Err(ModelListError::Unsupported) };
                    let _ = w.record_models(r);
                }
                Op::Assign => { let _ = w.assign(Assignments::default()); }
                Op::Limit(on) => {
                    let _ = w.set_cost_limit(on.then_some(CostLimit { enabled: true, monthly_limit_grosze: 100 }));
                }
                Op::Back => {
                    w.back();
                    if matches!(w.step(), WizardStep::EnterKey | WizardStep::ChooseProvider) {
                        tested_ok = false;
                    }
                }
                Op::Finish => {
                    match w.clone().finish() {
                        Ok(out) => {
                            prop_assert!(tested_ok);
                            prop_assert_eq!(Some(out.account.secret.expose_secret()), current_key);
                            prop_assert!(out.test.outcome.key_works());
                        }
                        Err(e) => {
                            let expected = matches!(
                                e,
                                WizardError::WrongStep { .. } | WizardError::TestNotPassed
                            );
                            prop_assert!(expected, "{}", e);
                        }
                    }
                }
            }
            let dump = format!("{w:?}");
            for key in &KEYS[..2] {
                prop_assert!(!dump.contains(key), "{}", dump);
            }
        }
    }
}

#[test]
fn provider_rules() {
    let catalog = fixture_catalog();
    let mut w = Wizard::new();
    assert!(matches!(
        w.choose_provider(catalog[1].clone()),
        Err(WizardError::ForbiddenProvider(_))
    ));
    assert!(matches!(
        w.choose_provider(catalog[3].clone()),
        Err(WizardError::CliLoginRequired(_))
    ));
    w.choose_provider(catalog[2].clone()).unwrap();
    assert_eq!(w.warnings().len(), 1);
    assert!(matches!(
        w.enter_key(SecretString::from("sk-ok-1"), "x", None),
        Err(WizardError::InvalidBaseUrl(_))
    ));
    assert!(matches!(
        w.enter_key(SecretString::from("sk-ok-1"), "x", Some("ftp://x".into())),
        Err(WizardError::InvalidBaseUrl(_))
    ));
    w.enter_key(
        SecretString::from("sk-ok-1"),
        "x",
        Some("https://my.example/v1".into()),
    )
    .unwrap();
    assert_eq!(w.base_url(), Some("https://my.example/v1"));
    assert_eq!(w.step(), WizardStep::TestConnection);
    w.back();
    assert_eq!(w.step(), WizardStep::EnterKey);
    w.back();
    assert_eq!(w.step(), WizardStep::ChooseProvider);
    assert!(w.provider().is_none());
}
