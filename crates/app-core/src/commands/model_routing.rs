//! Dobór modelu według trudności (zgłoszenie właściciela 2026-10-14): łatwe zadanie — lekki
//! model, kod — średni (np. lokalny Qwen 27B), trudne — mocny (GPT Sol, Opus); Krytyczka
//! sprawdza na lekkim, odrzucona odpowiedź idzie szczebel wyżej. Ustawienia w `model-routing.json`
//! katalogu konfiguracji; Router nakłada je od następnej odpowiedzi.

use crate::core::AppCore;
use crate::dto::{ModelRouting, ModelRoutingSettings, RoutingPreview};
use crate::error::AppError;

/// Najdłuższy tekst podglądu (znaki) — podgląd ocenia prośbę, nie dokument.
const PREVIEW_MAX: usize = 20_000;

fn no_router() -> AppError {
    AppError::unavailable("Dobór modelu", "router")
}

impl AppCore {
    /// `routing_get`: ustawienia doboru i modele Routera z ich szczeblami.
    pub async fn routing_get(&self) -> Result<ModelRouting, AppError> {
        self.inner.brain.routing().ok_or_else(no_router)
    }

    /// `routing_set`: zapisuje ustawienia doboru (szczeble ręczne w zapisie `dostawca:model`).
    pub async fn routing_set(
        &self,
        settings: ModelRoutingSettings,
    ) -> Result<ModelRouting, AppError> {
        self.inner.brain.routing().ok_or_else(no_router)?;
        self.inner
            .brain
            .set_routing(settings)
            .map_err(AppError::invalid)
    }

    /// `routing_preview`: jak Router potraktuje przykładowe zadanie w klasie (`chat`, `code`, …).
    pub async fn routing_preview(
        &self,
        text: String,
        task_class: String,
    ) -> Result<RoutingPreview, AppError> {
        super::accounts::task_from_name(&task_class)?;
        if text.chars().count() > PREVIEW_MAX {
            return Err(AppError::invalid(format!(
                "Przykład jest za długi (najwyżej {PREVIEW_MAX} znaków)."
            )));
        }
        self.inner.brain.routing().ok_or_else(no_router)?;
        self.inner
            .brain
            .routing_preview(&text, &task_class)
            .ok_or_else(no_router)
    }
}
