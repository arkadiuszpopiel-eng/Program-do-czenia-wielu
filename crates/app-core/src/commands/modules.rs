//! Komendy Ustawienia → Moduły (`modules_*`): lista modułów i wyłączanie opcjonalnych
//! (od następnego uruchomienia). Logika widoku: `app_modules::modules`.

use app_modules::modules::{DISABLED_KEY, can_disable, disabled_value, parse_disabled, view};

use crate::core::AppCore;
use crate::dto::ModulesView;
use crate::error::AppError;

impl AppCore {
    /// `modules_list`: moduły z manifestów i rejestru.
    pub async fn modules_list(&self) -> Result<ModulesView, AppError> {
        let disabled = parse_disabled(self.config_value(DISABLED_KEY).await.as_ref());
        let statuses = self.modules().await;
        Ok(view(
            &crate::compose::manifests(),
            &statuses,
            &disabled,
            &self.inner.extra.skipped,
        ))
    }

    /// `modules_set_enabled`: włącza / wyłącza moduł opcjonalny (zmiana po ponownym uruchomieniu).
    pub async fn modules_set_enabled(
        &self,
        id: String,
        enabled: bool,
    ) -> Result<ModulesView, AppError> {
        if !can_disable(&id) {
            return Err(AppError::forbidden(format!(
                "Modułu `{id}` nie można wyłączyć — jest niezbędny albo należy do Jądra."
            )));
        }
        let mut disabled = parse_disabled(self.config_value(DISABLED_KEY).await.as_ref());
        if enabled {
            disabled.remove(&id);
        } else {
            disabled.insert(id);
        }
        self.config_set(DISABLED_KEY, Some(disabled_value(&disabled)), true)
            .await?;
        self.modules_list().await
    }
}
