//! Przegląd 2026-10-21: wynik testu klucza bez rozstrzygnięcia (brak sieci, limit czasu) nie
//! wyłącza konta, którego klucz działał w poprzednim teście; konto bez takiego testu dostaje
//! stan błędu jak dotąd.

use accounts_hub_contract::{AccountErrorKind, AccountState, TestOutcome};

#[test]
fn inconclusive_test_keeps_a_working_account_active() {
    let network = TestOutcome::Network {
        message: "connection reset".into(),
    };
    let worked = Some(&TestOutcome::Ok);
    for outcome in [network.clone(), TestOutcome::Timeout] {
        assert_eq!(
            outcome.next_state_after(&AccountState::Active, worked),
            AccountState::Active
        );
        assert_eq!(
            outcome.next_state_after(&AccountState::Disabled, worked),
            AccountState::Disabled
        );
        // Nowe konto (bez udanego testu) — błąd jak dotąd.
        assert!(matches!(
            outcome.next_state_after(&AccountState::Active, None),
            AccountState::Error { .. }
        ));
    }
    assert_eq!(
        network.next_state_after(&AccountState::Unconfigured, worked),
        AccountState::Error {
            kind: AccountErrorKind::Network
        }
    );
    // Zły klucz nadal wyłącza konto.
    assert_eq!(
        TestOutcome::InvalidKey.next_state_after(&AccountState::Active, worked),
        AccountState::Error {
            kind: AccountErrorKind::InvalidKey
        }
    );
}
