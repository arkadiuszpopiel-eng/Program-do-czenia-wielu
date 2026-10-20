//! Okresowe sprawdzanie kluczy API w tle (plan v6, zadanie C2.14): o wygasłym kluczu albo braku
//! środków użytkownik dowiaduje się przed rozmową, nie w jej trakcie. Sprawdzenie to ten sam test
//! co przycisk „Testuj” — pytanie o listę modeli, bez zużycia tokenów. Konta wyłączone i bez
//! klucza są pomijane; bez sieci nic nie jest sprawdzane (wynik „brak sieci” nic nie mówi o kluczu).

use std::time::Duration;

use accounts_hub_contract::{
    Account as CoreAccount, AccountId, AccountState as CoreState, AccountsHub,
};
use app_models::accounts::{MAX_INTERVAL_HOURS, is_due};
use chrono::{DateTime, Utc};

use crate::core::AppCore;
use crate::dto::{AlfaEvent, LocalizedText, ToastKind};

/// Ustawienie: co ile godzin sprawdzać klucze (0 — nigdy).
pub(crate) const INTERVAL_KEY: &str = "accounts.check_interval_hours";
/// Wartość zalecana odstępu.
pub(crate) const DEFAULT_INTERVAL_HOURS: u64 = 24;
/// Pierwsze sprawdzenie po starcie (program ma najpierw wstać).
const FIRST_TICK: Duration = Duration::from_secs(120);
/// Jak często pętla patrzy, czy któreś konto jest już do sprawdzenia.
const TICK: Duration = Duration::from_secs(600);

/// Wynik jednego przebiegu sprawdzania.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct KeyCheckRound {
    /// Konta sprawdzone w tym przebiegu.
    pub checked: Vec<String>,
    /// Konta, których klucz działał, a teraz nie działa.
    pub broken: Vec<String>,
}

impl AppCore {
    /// Odstęp sprawdzania z ustawień (godziny; 0 — wyłączone).
    async fn key_check_interval(&self) -> u64 {
        self.config_value(INTERVAL_KEY)
            .await
            .and_then(|value| value.as_u64())
            .unwrap_or(DEFAULT_INTERVAL_HOURS)
            .min(MAX_INTERVAL_HOURS)
    }

    /// Jeden przebieg: sprawdza konta, którym minął odstęp. Konto, które przestało działać,
    /// dostaje komunikat (raz — przy zmianie z „działa” na „nie działa”).
    pub async fn check_keys_due(&self, now: DateTime<Utc>) -> KeyCheckRound {
        let mut round = KeyCheckRound::default();
        let interval = self.key_check_interval().await;
        if interval == 0 || !self.rt().online {
            return round;
        }
        let due: Vec<CoreAccount> = self
            .inner
            .hub
            .accounts()
            .into_iter()
            .filter(|account| is_due(account, now, interval))
            .collect();
        for before in due {
            let id: AccountId = before.id.clone();
            let worked = matches!(before.state, CoreState::Active);
            let tested = self.inner.hub.test_account(&id).await;
            self.announce_account(&id).await;
            round.checked.push(id.to_string());
            let works = match &tested {
                Ok(summary) => summary.outcome.key_works(),
                Err(_) => false,
            };
            // Brak sieci albo limit zapytań w chwili testu nie znaczy, że klucz jest zły.
            let inconclusive = tested
                .as_ref()
                .is_ok_and(|s| matches!(s.outcome.code(), "network" | "timeout" | "rate_limited"));
            if worked && !works && !inconclusive {
                round.broken.push(id.to_string());
                let label = &before.label;
                self.emit(AlfaEvent::Toast {
                    kind: ToastKind::Warning,
                    message: LocalizedText::new(
                        format!(
                            "Klucz konta „{label}” przestał działać. Szczegóły: Ustawienia → Konta i klucze API."
                        ),
                        format!(
                            "The key of account “{label}” stopped working. Details: Settings → Accounts and API keys."
                        ),
                    ),
                });
            }
        }
        round
    }

    /// Pętla sprawdzania kluczy w tle.
    pub(crate) fn spawn_key_watch(&self) {
        let core = self.clone();
        tokio::spawn(async move {
            tokio::time::sleep(FIRST_TICK).await;
            loop {
                let round = core.check_keys_due(Utc::now()).await;
                if !round.checked.is_empty() {
                    tracing::info!(
                        sprawdzone = round.checked.len(),
                        zepsute = round.broken.len(),
                        "okresowe sprawdzenie kluczy API"
                    );
                }
                tokio::time::sleep(TICK).await;
            }
        });
    }
}
