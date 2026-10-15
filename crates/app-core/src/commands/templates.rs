//! Komendy szablonów promptów (`prompt_templates_*`; walidacja: `app_modules::prompt_templates`)
//! i widoku „gdzie trafiają dane” (`compliance_routes`, `app_modules::data_routes`).

use app_modules::prompt_templates as pt;

use crate::core::AppCore;
use crate::dto::PromptTemplate;
use crate::error::AppError;

impl AppCore {
    /// `prompt_templates_get`: szablony promptów.
    pub async fn prompt_templates_get(&self) -> Result<Vec<PromptTemplate>, AppError> {
        Ok(pt::parse(self.config_value(pt::KEY).await.as_ref()))
    }

    /// `compliance_routes`: „gdzie trafiają dane” — trasy z rejestru zgodności (tylko odczyt).
    pub async fn compliance_routes(&self) -> Result<crate::dto::DataRoutesView, AppError> {
        Ok(app_modules::data_routes::view(
            self.inner._compliance.as_ref(),
        ))
    }

    /// `prompt_templates_set`: zapis całej listy po walidacji.
    pub async fn prompt_templates_set(
        &self,
        templates: Vec<PromptTemplate>,
    ) -> Result<Vec<PromptTemplate>, AppError> {
        self.config_set(pt::KEY, Some(pt::validate(templates)?), false)
            .await?;
        self.prompt_templates_get().await
    }
}
