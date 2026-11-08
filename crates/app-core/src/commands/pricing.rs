//! Komendy `pricing_*`: cennik modeli (Ustawienia → Cennik modeli) — logika w `app_models::pricing`.

use crate::core::AppCore;
use crate::dto::PricingView;
use crate::error::AppError;

/// Plik cennika użytkownika w katalogu konfiguracji (czyta go też Router — `RouterBrain`).
pub(crate) const FILE: &str = app_models::pricing::FILE;

impl AppCore {
    /// `pricing_get`.
    pub async fn pricing_get(&self) -> Result<PricingView, AppError> {
        Ok(app_models::pricing::get(
            &self.inner.paths.config.join(FILE),
        ))
    }

    /// `pricing_apply` — `save = false`: podgląd zmian; `true`: zapis (Router bierze nowe ceny
    /// przy najbliższej synchronizacji kont, bez restartu).
    pub async fn pricing_apply(&self, text: String, save: bool) -> Result<PricingView, AppError> {
        let path = self.inner.paths.config.join(FILE);
        app_models::pricing::apply(&path, &text, save).map_err(AppError::invalid)
    }
}
