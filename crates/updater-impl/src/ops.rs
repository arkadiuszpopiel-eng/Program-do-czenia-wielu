//! Operacje `FsUpdater` poza traitem kontraktu (F3): przełączenie na przygotowaną aktualizację
//! z zachowaniem uruchomionej wersji jako poprzedniej, rollback na żądanie użytkownika albo
//! watchdoga (wersja porzucona trafia do wycofanych), sprzątanie bez uruchomionej wersji,
//! przyjęcie wersji z instalatora (`alfa.exe --alfa-installed <ver>`).

use chrono::Utc;
use semver::Version;
use updater_contract::{
    CurrentState, STATE_SCHEMA, Updater, UpdaterError, VERSION_FILE, events as ev, prune_victims,
};

use crate::{FsUpdater, store};

impl FsUpdater {
    /// Aktywuje rozpakowaną wersję `to` (czeka na `mark_good`). Poprzednią zostaje `running`
    /// (sprawdzona, uruchomiona wersja), także gdy wcześniej przygotowano inną, jeszcze nie
    /// uruchomioną aktualizację — rollback zawsze wraca do czegoś, co działało.
    pub fn activate_staged(
        &self,
        to: &Version,
        running: &Version,
    ) -> Result<CurrentState, UpdaterError> {
        if !self.usable(to) {
            return Err(UpdaterError::NotInstalled {
                version: to.to_string(),
            });
        }
        let before = store::read_state(self.layout());
        let now = Utc::now();
        let mut after = match &before {
            Some(s) => s.switched(to, now),
            None => CurrentState {
                pending: true,
                ..CurrentState::initial(to.clone(), now)
            },
        };
        let previous_unproven = before
            .as_ref()
            .is_none_or(|b| b.pending && b.active != *running);
        if to != running && previous_unproven && self.usable(running) {
            after.previous = Some(running.clone());
        }
        after.schema = STATE_SCHEMA;
        self.save(before.as_ref(), &after)?;
        let from = before.map(|s| s.active.to_string());
        self.outbox.emit(
            ev::SWITCHED,
            serde_json::json!({ "from": from, "to": to.to_string() }),
        );
        Ok(after)
    }

    /// Rollback na żądanie (użytkownik w UI albo watchdog po pętli awarii): aktywna ↔ poprzednia,
    /// a porzucona wersja trafia do wycofanych (nie wraca automatycznie; zdejmuje to dopiero
    /// jawne `switch_to`). Zwraca nową aktywną.
    pub fn rollback_by(&self, reason: &str) -> Result<Version, UpdaterError> {
        let before = store::read_state(self.layout()).ok_or(UpdaterError::NoPrevious)?;
        let mut after = before.rolled_back(Utc::now())?;
        if !self.usable(&after.active) {
            return Err(UpdaterError::NotInstalled {
                version: after.active.to_string(),
            });
        }
        if !after.bad.contains(&before.active) {
            after.bad.push(before.active.clone());
        }
        self.save(Some(&before), &after)?;
        let payload = serde_json::json!({ "from": before.active.to_string(), "to": after.active.to_string(), "reason": reason });
        self.outbox.emit(ev::ROLLED_BACK, payload);
        Ok(after.active)
    }

    /// Sprzątanie jak `prune`, ale nigdy nie usuwa wersji z `protect` (uruchomionej).
    pub fn prune_except(
        &self,
        keep: usize,
        protect: &[Version],
    ) -> Result<Vec<Version>, UpdaterError> {
        let state = store::read_state(self.layout());
        let victims: Vec<Version> =
            prune_victims(&store::version_dirs(self.layout()), state.as_ref(), keep)
                .into_iter()
                .filter(|v| !protect.contains(v))
                .collect();
        for v in &victims {
            store::remove_version(self.layout(), v)?;
        }
        if !victims.is_empty() {
            let removed: Vec<String> = victims.iter().map(ToString::to_string).collect();
            self.outbox
                .emit(ev::PRUNED, serde_json::json!({ "removed": removed }));
        }
        Ok(victims)
    }

    /// Wersja skopiowana przez instalator NSIS do `versions\<ver>\`: zapisuje `version.json`,
    /// przełącza na nią (czeka na `mark_good`; przy awarii wraca poprzednia) i sprząta.
    pub fn adopt_installed(&self, version: &Version) -> Result<CurrentState, UpdaterError> {
        let dir = self.layout().version_dir(version);
        let file = dir.join(VERSION_FILE);
        if self.layout().app_exe(version).is_file() && !file.exists() {
            let json = serde_json::json!({ "version": version.to_string() });
            store::write_atomic(&file, json.to_string().as_bytes())?;
        }
        let state = self.switch_to(version)?;
        self.prune_except(self.config().keep_versions, &[])?;
        Ok(state)
    }
}
