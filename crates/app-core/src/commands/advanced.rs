//! Komendy Ustawienia → Zaawansowane (`config_raw_*`): surowy edytor plików konfiguracji
//! z walidacją magazynu (`app_modules::raw_config`).

use app_modules::raw_config::{RawLayer, check, get, save};

use crate::core::AppCore;
use crate::dto::RawConfig;
use crate::error::AppError;

impl AppCore {
    /// `config_raw_get`: treść `shared.toml` i nakładki maszyny.
    pub async fn config_raw_get(&self) -> Result<RawConfig, AppError> {
        Ok(get(&self.inner.config, &self.inner.machine.0))
    }

    /// `config_raw_check`: kontrola składni (liczba kluczy albo błąd z wierszem).
    pub async fn config_raw_check(&self, text: String) -> Result<u32, AppError> {
        check(&text)
            .map(|n| u32::try_from(n).unwrap_or(u32::MAX))
            .map_err(|e| AppError::invalid_field("text", format!("Błąd składni TOML: {e}.")))
    }

    /// `config_raw_save`: zapis warstwy z pełną walidacją; odmowa przywraca plik.
    pub async fn config_raw_save(
        &self,
        layer: String,
        text: String,
    ) -> Result<RawConfig, AppError> {
        let layer = RawLayer::parse(&layer)?;
        save(&self.inner.config, &self.inner.machine.0, layer, &text).await?;
        self.config_raw_get().await
    }
}
