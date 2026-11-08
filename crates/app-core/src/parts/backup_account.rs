//! Konto zapasowe (plan v6, zadanie C2.15): gdy konto dostawcy odpowiada limitem zapytań albo
//! odrzuca zapytania (klucz, brak środków), kolejne wiadomości idą przez inne używalne konto
//! tego samego dostawcy — bez udziału użytkownika, z komunikatem. Bez drugiego konta nic się
//! nie zmienia. Router sam kończy bieżącą turę na koncie zapasowym (trasa konta w kolejce),
//! a rdzeń po `router.fallback` odstawia konto i mówi o przełączeniu (decyzja 27.10).

use std::time::Instant;

use core_bus_contract::{BusItem, EventFilter};
use futures_util::StreamExt;

use accounts_hub_contract::{AccountId, AccountsHub};
pub(crate) use app_files::projects::Retests;
use app_files::projects::{bench_until, handover, switched_toast as switched};

use crate::core::AppCore;
use crate::dto::{TurnError, TurnErrorCode};

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
                let until = bench_until(error.retry_at.as_deref());
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
                // Test jak „Testuj”; komunikat według stanu konta po teście (zły klucz, brak środków).
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

impl AppCore {
    /// `router.fallback` między dwoma kontami tego samego dostawcy: Router sam przełączył turę na
    /// konto zapasowe — rdzeń odstawia konto (limit) albo je sprawdza (odrzucenie) i mówi o tym.
    pub(crate) async fn spawn_fallback_bridge(&self) {
        let stream = self
            .inner
            .bus
            .subscribe(EventFilter::prefix("router.fallback"))
            .await;
        let Ok(mut stream) = stream else {
            tracing::warn!("most przełączeń Routera niedostępny");
            return;
        };
        let core = self.clone();
        tokio::spawn(async move {
            while let Some(item) = stream.next().await {
                let BusItem::Event(event) = item else {
                    continue;
                };
                let target = |m: &str| core.inner.brain.target(m);
                if let Some((account, error)) = handover(&event.payload, target) {
                    core.account_trouble(&account, &error).await;
                }
            }
        });
    }
}
