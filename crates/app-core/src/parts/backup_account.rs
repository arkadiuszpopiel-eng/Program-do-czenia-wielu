//! Konto zapasowe (plan v6, zadanie C2.15): gdy konto dostawcy odpowiada limitem zapytań albo
//! odrzuca zapytania (klucz, brak środków), kolejne wiadomości idą przez inne używalne konto
//! tego samego dostawcy — bez udziału użytkownika, z komunikatem. Bez drugiego konta nic się
//! nie zmienia. Tura, która trafiła na błąd, nie jest powtarzana sama: użytkownik ją ponawia.

use std::collections::HashMap;
use std::sync::{Mutex, PoisonError};
use std::time::{Duration, Instant};

use accounts_hub_contract::{AccountId, AccountsHub};
use chrono::Utc;

use crate::core::AppCore;
use crate::dto::{AlfaEvent, LocalizedText, ToastKind, TurnError, TurnErrorCode};

/// Odstawienie konta, gdy dostawca nie podał, kiedy limit mija.
const DEFAULT_BENCH: chrono::Duration = chrono::Duration::seconds(60);
/// Najdłuższe odstawienie (dostawca potrafi podać dobę — po godzinie i tak próbujemy).
const MAX_BENCH: chrono::Duration = chrono::Duration::hours(1);
/// Najkrótszy odstęp między sprawdzeniami tego samego konta po błędach w rozmowie.
const RETEST_EVERY: Duration = Duration::from_secs(60);

/// Kiedy ostatnio sprawdzano konto po błędzie w rozmowie (konto → chwila).
#[derive(Default)]
pub(crate) struct Retests(Mutex<HashMap<String, Instant>>);

impl Retests {
    /// `true` — wolno sprawdzić teraz (i zapisuje tę chwilę).
    fn allow(&self, account: &str, now: Instant) -> bool {
        let mut seen = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        if seen
            .get(account)
            .is_some_and(|last| now.duration_since(*last) < RETEST_EVERY)
        {
            return false;
        }
        seen.insert(account.to_owned(), now);
        true
    }
}

fn switched(from: &str, to: &str, why_pl: &str, why_en: &str) -> AlfaEvent {
    AlfaEvent::Toast {
        kind: ToastKind::Info,
        message: LocalizedText::new(
            format!(
                "Konto „{from}” {why_pl} — kolejne wiadomości pójdą przez konto zapasowe „{to}”."
            ),
            format!(
                "Account “{from}” {why_en} — next messages will go through the backup account “{to}”."
            ),
        ),
    }
}

impl AppCore {
    /// Tura skończyła się błędem dostawcy na koncie `account`.
    pub(crate) async fn account_trouble(&self, account: &str, error: &TurnError) {
        let Ok(id) = AccountId::new(account) else {
            return;
        };
        let Some(before) = self.inner.hub.account(&id) else {
            return;
        };
        match error.code {
            TurnErrorCode::RateLimited => {
                let now = Utc::now();
                let until = error
                    .retry_at
                    .as_deref()
                    .and_then(|t| chrono::DateTime::parse_from_rfc3339(t).ok())
                    .map_or(now + DEFAULT_BENCH, |t| t.with_timezone(&Utc))
                    .clamp(now, now + MAX_BENCH);
                if let Some(backup) = self.inner.brain.bench_account(account, until) {
                    tracing::info!(konto = %id, "limit zapytań — konto zapasowe przejmuje");
                    self.emit(switched(
                        &before.label,
                        &backup,
                        "ma limit zapytań",
                        "is rate limited",
                    ));
                }
            }
            TurnErrorCode::Provider => {
                // Bez konta zapasowego nie ma na co przełączać — stan konta pokaże zwykły test.
                if self.inner.brain.backup_for(account).is_none()
                    || !self.inner.retests.allow(account, Instant::now())
                {
                    return;
                }
                // Ten sam test co przycisk „Testuj”: odrzucony klucz albo brak środków zmienia stan
                // konta i Router bierze następne. Komunikat według stanu po teście, nie wyniku.
                let core = self.clone();
                let label = before.label.clone();
                tokio::spawn(async move {
                    let tested = core.inner.hub.test_account(&id).await;
                    core.announce_account(&id).await;
                    let broken = tested.is_ok()
                        && core
                            .inner
                            .hub
                            .account(&id)
                            .is_some_and(|a| !a.state.is_usable());
                    let backup = core.inner.brain.backup_for(id.as_str());
                    if let (true, Some(backup)) = (broken, backup) {
                        tracing::info!(konto = %id, "konto odrzuca zapytania — konto zapasowe przejmuje");
                        core.emit(switched(
                            &label,
                            &backup,
                            "odrzuca zapytania (klucz albo środki)",
                            "rejects requests (key or credit)",
                        ));
                    }
                });
            }
            _ => {}
        }
    }
}
