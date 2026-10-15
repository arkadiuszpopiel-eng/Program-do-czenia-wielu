//! Komendy Ustawienia → Polecenia bez modelu (`quick_commands_*`): własne polecenia, katalog
//! wbudowanych, tester. Logika: `app_modules::quick_commands`.

use app_modules::quick_commands as qc;

use crate::core::AppCore;
use crate::dto::{CustomCommand, QuickCommandsView, QuickTest};
use crate::error::AppError;

impl AppCore {
    /// `quick_commands_get`: własne polecenia i katalog wbudowanych.
    pub async fn quick_commands_get(&self) -> Result<QuickCommandsView, AppError> {
        let enabled = self.config_bool(lib_quick::keys::ENABLED, true).await;
        Ok(qc::view(self.config_value(qc::KEY).await.as_ref(), enabled))
    }

    /// `quick_commands_set`: zapis całej listy po walidacji.
    pub async fn quick_commands_set(
        &self,
        commands: Vec<CustomCommand>,
    ) -> Result<QuickCommandsView, AppError> {
        self.config_set(qc::KEY, Some(qc::validate(commands)?), false)
            .await?;
        self.quick_commands_get().await
    }

    /// `quick_commands_test`: kto odpowie na wiadomość (własne, quiz, wbudowane, model).
    pub async fn quick_commands_test(&self, message: String) -> Result<QuickTest, AppError> {
        let enabled = self.config_bool(lib_quick::keys::ENABLED, true).await;
        let value = self.config_value(qc::KEY).await;
        Ok(qc::test(value.as_ref(), enabled, &message))
    }
}
