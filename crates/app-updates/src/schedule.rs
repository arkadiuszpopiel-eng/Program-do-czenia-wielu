//! Zadania w tle: `mark_good` po zdrowym starcie (nowa wersja przestaje czekać na potwierdzenie,
//! launcher kończy obserwację; przygotowuje się nowy launcher z paczki) i sprawdzanie
//! aktualizacji wg trybu — „automatycznie” i „pytaj” raz na dobę (pierwsze po kilku minutach od
//! startu), „automatycznie” od razu pobiera i przygotowuje; „ręcznie” — nigdy samo.

use std::sync::{Arc, Weak};
use std::time::Duration;

use updater_contract::{InstallIntent, UpdateMode, UpdatePhase, Updater};

use crate::UpdatesApp;

/// Harmonogram zadań w tle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Schedule {
    /// Po jakim czasie bez awarii start jest zdrowy (`mark_good`).
    pub healthy_after: Duration,
    /// Pierwsze sprawdzenie po starcie.
    pub first_check_after: Duration,
    /// Co ile sprawdzać, czy minęła doba od ostatniego sprawdzenia.
    pub tick: Duration,
    /// Odstęp między automatycznymi sprawdzeniami.
    pub check_every: Duration,
}

impl Default for Schedule {
    fn default() -> Self {
        Self {
            healthy_after: Duration::from_secs(30),
            first_check_after: Duration::from_secs(5 * 60),
            tick: Duration::from_secs(60 * 60),
            check_every: Duration::from_secs(24 * 60 * 60),
        }
    }
}

impl UpdatesApp {
    /// Uruchamia zadania w tle (trzymają tylko słabe odwołanie — kończą się z aplikacją).
    pub fn spawn_background(self: &Arc<Self>, schedule: Schedule) {
        let weak = Arc::downgrade(self);
        tokio::spawn(mark_good_after(weak.clone(), schedule.healthy_after));
        tokio::spawn(check_loop(weak, schedule));
    }

    /// Jedno sprawdzenie wg harmonogramu (`true` — sprawdzono).
    pub async fn scheduled_check(&self, check_every: Duration) -> bool {
        let mode = self.sync_prefs().await;
        let status = self.service.status();
        let due = status.last_check.is_none_or(|t| {
            chrono::Utc::now()
                .signed_duration_since(t)
                .to_std()
                .is_ok_and(|age| age >= check_every)
        });
        if !mode.checks_automatically()
            || !due
            || status.busy()
            || status.phase == UpdatePhase::Disabled
        {
            return false;
        }
        match self.service.check().await {
            Ok(s) if s.phase == UpdatePhase::Available && mode == UpdateMode::Auto => {
                if let Err(e) = self.service.download(InstallIntent::Update).await {
                    tracing::warn!(error = %e, "automatyczna aktualizacja nie powiodła się");
                }
            }
            Ok(_) => {}
            Err(e) => tracing::info!(error = %e, "sprawdzanie aktualizacji nie powiodło się"),
        }
        true
    }
}

async fn mark_good_after(app: Weak<UpdatesApp>, after: Duration) {
    tokio::time::sleep(after).await;
    let Some(app) = app.upgrade() else {
        return;
    };
    let updater = app.service.updater().clone();
    let version = app.service.status().current;
    let result = tokio::task::spawn_blocking(move || match updater.state() {
        Ok(Some(state)) if state.active == version && state.pending => {
            updater.mark_good(&version).map(|()| true)
        }
        Ok(_) => Ok(false),
        Err(e) => Err(e),
    })
    .await;
    match result {
        Ok(Ok(true)) => {
            tracing::info!("zdrowy start — wersja oznaczona jako dobra");
            app.service.refresh_local();
        }
        Ok(Ok(false)) => tracing::debug!("mark_good: brak oczekującej wersji"),
        Ok(Err(e)) => tracing::warn!(error = %e, "mark_good nie powiódł się"),
        Err(e) => tracing::warn!(error = %e, "zadanie mark_good przerwane"),
    }
}

async fn check_loop(app: Weak<UpdatesApp>, schedule: Schedule) {
    tokio::time::sleep(schedule.first_check_after).await;
    loop {
        let Some(strong) = app.upgrade() else {
            return;
        };
        strong.scheduled_check(schedule.check_every).await;
        drop(strong);
        tokio::time::sleep(schedule.tick).await;
    }
}
