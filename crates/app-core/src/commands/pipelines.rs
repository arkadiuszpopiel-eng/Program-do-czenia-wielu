//! Pipeline'y modeli (`pipelines_*`): biblioteka własnych łańcuchów etapów (rola + model + zadanie),
//! które Dyrygentka uruchamia narzędziem `run_pipeline` bez pośrednictwa właściciela.
//! Własne pipeline'y są w konfiguracji (`pipelines.saved`); wbudowane wzorce są tylko do odczytu.

use app_chat::pipelines as pl;

use crate::core::AppCore;
use crate::dto::PipelineDef;
use crate::error::AppError;

impl AppCore {
    async fn saved_pipelines(&self) -> Vec<PipelineDef> {
        pl::parse_saved(self.config_value(pl::SAVED_KEY).await.as_ref())
    }

    async fn store_pipelines(&self, saved: &[PipelineDef]) -> Result<Vec<PipelineDef>, AppError> {
        self.config_set(pl::SAVED_KEY, Some(pl::encode_saved(saved).into()), false)
            .await?;
        Ok(pl::library(saved))
    }

    /// `pipelines_list`: własne pipeline'y i wzorce wbudowane, których nikt nie nadpisał.
    pub async fn pipelines_list(&self) -> Result<Vec<PipelineDef>, AppError> {
        Ok(pl::library(&self.saved_pipelines().await))
    }

    /// `pipelines_save`: dodaje lub aktualizuje własny pipeline (`previous_name` — zmiana nazwy).
    pub async fn pipelines_save(
        &self,
        pipeline: PipelineDef,
        previous_name: Option<String>,
    ) -> Result<Vec<PipelineDef>, AppError> {
        let clean = pl::validate(&pipeline).map_err(AppError::invalid)?;
        let saved = self.saved_pipelines().await;
        let next =
            pl::upsert(&saved, clean, previous_name.as_deref()).map_err(AppError::invalid)?;
        self.store_pipelines(&next).await
    }

    /// `pipelines_remove`: usuwa własny pipeline (nadpisany wzorzec wraca do wbudowanego).
    pub async fn pipelines_remove(&self, name: String) -> Result<Vec<PipelineDef>, AppError> {
        let saved = self.saved_pipelines().await;
        self.store_pipelines(&pl::remove(&saved, &name)).await
    }
}
