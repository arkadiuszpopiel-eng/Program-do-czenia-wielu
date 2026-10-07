//! Działania na pakietach 1–6 (`models_bundles`, `models_bundle_download`, `models_bundle_verify`)
//! i naprawa jednej pozycji (`models_repair`) — złożone z istniejących prymitywów menedżera:
//! pobieranie (limit równoległości, wznawianie, zgoda TOFU), weryfikacja SHA-256, usuwanie.

use std::sync::Arc;

use app_api::dto::{ModelBundle, ModelItem, ModelItemState};
use app_api::{AppError, ErrorCode};

use crate::ModelsApp;
use crate::bundle_data::Def;
use crate::bundles::{self, Machine};
use crate::catalog::ItemSpec;

fn bundle_def(id: &str) -> Result<&'static Def, AppError> {
    bundles::def(id).ok_or_else(|| {
        AppError::not_found(format!(
            "Nie ma pakietu „{id}” — wybierz jeden z pakietów 1–6 z listy."
        ))
    })
}

impl ModelsApp {
    fn items_now(&self) -> Vec<ModelItem> {
        self.catalog.iter().map(|s| self.item(s)).collect()
    }

    fn bundle_view(&self, def: &Def, machine: &Machine) -> ModelBundle {
        let best = bundles::recommended(machine);
        bundles::view(def, machine, best == Some(def.rating), &self.items_now())
    }

    /// Pozycje pakietu obecne w katalogu (warianty silników dla tej maszyny).
    fn bundle_specs(&self, def: &Def, machine: &Machine) -> Vec<ItemSpec> {
        bundles::members(def, machine.accel())
            .into_iter()
            .filter_map(|(id, _)| self.catalog.iter().find(|s| s.id == id).cloned())
            .collect()
    }

    /// `models_bundles` — pakiety od 6 do 1 dopasowane do tej maszyny, ze stanem pozycji.
    pub async fn bundles(&self, machine: &Machine) -> Result<Vec<ModelBundle>, AppError> {
        Ok(bundles::views(machine, &self.items_now()))
    }

    /// `models_bundle_download` — pobiera w tle brakujące, wstrzymane i nieudane pozycje pakietu,
    /// a uszkodzone naprawia ([`Self::repair`]). Zainstalowanych, trwających i czekających na zgodę
    /// TOFU nie rusza; pozycji instalowanych ręcznie nie pobiera (widok: `downloadable: false`).
    /// Pozycja, której nie udało się uruchomić, dostaje błąd w swoim stanie (`failed`).
    pub async fn bundle_download(
        self: &Arc<Self>,
        bundle_id: &str,
        machine: &Machine,
    ) -> Result<ModelBundle, AppError> {
        let def = bundle_def(bundle_id)?;
        for spec in self.bundle_specs(def, machine) {
            if !spec.downloadable() {
                continue;
            }
            let started = match self.item(&spec).state {
                ModelItemState::Missing | ModelItemState::Paused | ModelItemState::Failed => {
                    self.download(&spec.id).await.map(drop)
                }
                ModelItemState::Corrupt => self.repair(&spec.id).await.map(drop),
                _ => Ok(()),
            };
            if let Err(e) = started {
                tracing::warn!(item = %spec.id, error = %e, "pozycja pakietu nie ruszyła");
                self.set_live(&spec.id, |l| l.error = Some(e.message.clone()));
                self.emit_item(&spec);
            }
        }
        Ok(self.bundle_view(def, machine))
    }

    /// `models_bundle_verify` — ponowne SHA-256 każdej zainstalowanej (także uszkodzonej) pozycji
    /// pakietu; niezgodny albo brakujący plik → pozycja `corrupt`, pakiet `corrupt`.
    pub async fn bundle_verify(
        &self,
        bundle_id: &str,
        machine: &Machine,
    ) -> Result<ModelBundle, AppError> {
        let def = bundle_def(bundle_id)?;
        for spec in self.bundle_specs(def, machine) {
            let state = self.item(&spec).state;
            if matches!(
                state,
                ModelItemState::Installed | ModelItemState::External | ModelItemState::Corrupt
            ) && let Err(e) = self.verify(&spec.id).await
            {
                tracing::warn!(item = %spec.id, error = %e, "weryfikacja pozycji pakietu pominięta");
            }
        }
        Ok(self.bundle_view(def, machine))
    }

    /// `models_repair` — „napraw ręcznie” jedną pozycję: przerywa pobieranie, usuwa pliki,
    /// częściowe pobrania i rekord instalacji, po czym pobiera ją od nowa (SHA-256 albo zgoda TOFU).
    pub async fn repair(self: &Arc<Self>, item_id: &str) -> Result<ModelItem, AppError> {
        let spec = self
            .catalog
            .iter()
            .find(|s| s.id == item_id)
            .cloned()
            .ok_or_else(|| {
                AppError::not_found(format!(
                    "Nie ma pozycji „{item_id}” w katalogu modeli i silników."
                ))
            })?;
        if !spec.downloadable() {
            return Err(AppError::invalid(format!(
                "„{}” instaluje się ręcznie — napraw ją według opisu pozycji (skopiuj pliki jeszcze raz).",
                spec.name
            )));
        }
        self.remove(&spec.id).await.map_err(|e| match e.code {
            ErrorCode::Storage => AppError::new(
                ErrorCode::Storage,
                format!(
                    "Nie udało się usunąć plików „{}” — zamknij to, co ich używa (np. trwającą rozmowę), i spróbuj ponownie. {}",
                    spec.name, e.message
                ),
            ),
            _ => e,
        })?;
        self.download(&spec.id).await
    }
}
