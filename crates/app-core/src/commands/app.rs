//! Komendy `app_*`: start UI, onboarding, układ okna, aktywna sesja, ustawienia Windows.

use std::collections::BTreeMap;

use crate::core::AppCore;
use crate::dto::{AppBootstrap, LayoutPrefs, Locale};
use crate::error::AppError;
use crate::settings::keys;

/// Strony ustawień Windows, które UI może otworzyć (lista dozwolonych w rdzeniu).
pub const SYSTEM_SETTINGS_ALLOWED: [&str; 2] =
    ["ms-settings:privacy-microphone", "ms-settings:storagesense"];

fn machine_name() -> String {
    ["COMPUTERNAME", "HOSTNAME"]
        .iter()
        .find_map(|v| std::env::var(v).ok().filter(|s| !s.trim().is_empty()))
        .unwrap_or_else(|| "Ten komputer".to_owned())
}

impl AppCore {
    /// Obiekt JSON zapisany w konfiguracji jako tekst (klucze dowolne — nie są kluczami TOML).
    pub(crate) async fn config_json<T: serde::de::DeserializeOwned>(&self, key: &str) -> Option<T> {
        let text = self.config_str(key).await?;
        serde_json::from_str(&text).ok()
    }

    /// Nadpisania skrótów.
    pub(crate) async fn shortcut_overrides(&self) -> BTreeMap<String, String> {
        self.config_json(keys::SHORTCUTS).await.unwrap_or_default()
    }

    /// `app_bootstrap`: język, ustawienia, układ (per maszyna), aktywna sesja, skróty, onboarding.
    pub async fn app_bootstrap(&self) -> Result<AppBootstrap, AppError> {
        let locale = match self.config_str(keys::LOCALE).await.as_deref() {
            Some("en") => Locale::En,
            _ => Locale::Pl,
        };
        let active = match self.config_str(keys::ACTIVE_SESSION).await {
            Some(id) => crate::ids::session(&id)
                .ok()
                .filter(|s| self.ensure_session(s).is_ok())
                .map(|s| s.to_string()),
            None => None,
        };
        Ok(AppBootstrap {
            app_version: self.inner.app_version.clone(),
            locale,
            onboarding_done: self.config_bool(keys::ONBOARDING_DONE, false).await,
            machine_name: machine_name(),
            settings: self.settings_values().await?,
            layout: self.config_json(keys::LAYOUT).await,
            active_session_id: active,
            shortcut_overrides: self.shortcut_overrides().await,
        })
    }

    /// `app_complete_onboarding`.
    pub async fn app_complete_onboarding(&self) -> Result<(), AppError> {
        self.config_set(keys::ONBOARDING_DONE, Some(true.into()), false)
            .await
    }

    /// `app_open_system_settings` ⟶ tylko adresy z listy dozwolonych.
    pub async fn app_open_system_settings(&self, uri: String) -> Result<(), AppError> {
        if !SYSTEM_SETTINGS_ALLOWED.contains(&uri.as_str()) {
            return Err(AppError::forbidden(format!(
                "Adres „{uri}” nie jest na liście dozwolonych stron ustawień Windows."
            )));
        }
        self.inner.shell.open_system_settings(&uri)
    }

    /// `app_save_layout` → nakładka maszyny.
    pub async fn app_save_layout(&self, layout: LayoutPrefs) -> Result<(), AppError> {
        let text = serde_json::to_string(&layout).map_err(AppError::internal)?;
        self.config_set(keys::LAYOUT, Some(text.into()), true).await
    }

    /// `app_set_active_session` → nakładka maszyny.
    pub async fn app_set_active_session(&self, session_id: Option<String>) -> Result<(), AppError> {
        let value = match session_id {
            Some(id) => {
                let id = crate::ids::session(&id)?;
                // Otwarcie skażonej sesji (np. po restarcie) — stan dla Brokera od razu; tura i tak
                // sprawdza to ponownie (fail-closed), więc tu tylko ostrzeżenie.
                if let Err(e) = self.chat().sync_taint(&id).await {
                    tracing::warn!(error = %e, "zgłoszenie skażenia sesji Brokerowi nie powiodło się");
                }
                Some(id.to_string().into())
            }
            None => None,
        };
        self.config_set(keys::ACTIVE_SESSION, value, true).await
    }

    /// „Przejdź do sesji" spoza okna głównego (zasobnik, protokół `alfa://session/…`,
    /// Szybkie pytanie): aktywna sesja + zdarzenie `OpenSession` (UI przełącza widok) + okno.
    pub async fn open_session_in_ui(&self, session_id: String) -> Result<(), AppError> {
        let id = crate::ids::session(&session_id)?;
        self.ensure_session(&id)?;
        self.app_set_active_session(Some(id.to_string())).await?;
        self.emit(crate::dto::AlfaEvent::OpenSession {
            session_id: id.to_string(),
        });
        self.inner.shell.show_main(Some(id.as_str()))
    }
}
