//! Komendy `models_local_*`: modele lokalne z manifestu `providers-local` (onboarding, Ustawienia →
//! Modele i dostawcy → Lokalne) i ich pobieranie z wznawianiem i SHA-256; postęp jako zdarzenia
//! `LocalModelProgress` (most z `local.model.download.progress` w `lifecycle`).

use std::sync::Arc;

use providers_contract::CancellationToken;
use providers_local_impl::{LocalError, LocalModule};

use crate::core::AppCore;
use crate::dto::{AlfaEvent, LocalDownloadState, LocalModelInfo};
use crate::error::AppError;

const MB: u64 = 1024 * 1024;

fn progress(
    model_id: &str,
    state: LocalDownloadState,
    bytes: u64,
    total: Option<u64>,
    error: Option<String>,
) -> AlfaEvent {
    AlfaEvent::LocalModelProgress {
        model_id: model_id.to_owned(),
        state,
        bytes,
        total,
        error,
    }
}

impl AppCore {
    fn local_module(&self) -> Result<Arc<LocalModule>, AppError> {
        self.inner
            .extra
            .local
            .clone()
            .ok_or_else(|| AppError::unavailable("Model lokalny", "providers-local"))
    }

    /// `models_local_list`.
    pub async fn models_local_list(&self) -> Result<Vec<LocalModelInfo>, AppError> {
        let provider = self.local_module()?.provider();
        let installed: Vec<String> = provider.installed().iter().map(|m| m.id.clone()).collect();
        let default = provider.sidecar().config().default_model.clone();
        let downloading: Vec<String> = self.rt().downloads.keys().cloned().collect();
        Ok(provider
            .models()
            .iter()
            .map(|m| LocalModelInfo {
                id: m.id.clone(),
                name: m.name.clone(),
                size_bytes: u64::from(m.size_mb) * MB,
                installed: installed.contains(&m.id),
                default: m.id == default,
                downloading: downloading.contains(&m.id),
            })
            .collect())
    }

    /// `models_local_download` (`None` = model domyślny); postęp przez `LocalModelProgress`.
    pub async fn models_local_download(&self, model_id: Option<String>) -> Result<(), AppError> {
        let module = self.local_module()?;
        let provider = module.provider();
        let id = model_id.unwrap_or_else(|| provider.sidecar().config().default_model.clone());
        let entry = provider
            .entry(&id)
            .cloned()
            .ok_or_else(|| AppError::invalid(format!("Nieznany model lokalny „{id}”.")))?;
        let total = Some(u64::from(entry.size_mb) * MB);
        if provider.installed().iter().any(|m| m.id == id) {
            self.emit(progress(&id, LocalDownloadState::Done, 0, total, None));
            return Ok(());
        }
        let cancel = {
            let mut rt = self.rt();
            if rt.downloads.contains_key(&id) {
                return Ok(());
            }
            let cancel = CancellationToken::new();
            rt.downloads.insert(id.clone(), cancel.clone());
            cancel
        };
        self.emit(progress(
            &id,
            LocalDownloadState::Downloading,
            0,
            total,
            None,
        ));
        let core = self.clone();
        tokio::spawn(async move {
            let result = module.download(&id, &cancel).await;
            core.rt().downloads.remove(&id);
            let event = match result {
                Ok(done) => {
                    let bytes = std::fs::metadata(&done.path).map_or(0, |m| m.len());
                    progress(&id, LocalDownloadState::Done, bytes, Some(bytes), None)
                }
                Err(LocalError::Cancelled) => {
                    progress(&id, LocalDownloadState::Cancelled, 0, total, None)
                }
                Err(e) => progress(
                    &id,
                    LocalDownloadState::Failed,
                    0,
                    total,
                    Some(format!("Pobieranie modelu nie powiodło się: {e}")),
                ),
            };
            core.emit(event);
            let status = core.status_snapshot().await;
            core.emit(AlfaEvent::SystemStatusChanged { status });
        });
        Ok(())
    }

    /// `models_local_cancel` (`None` = wszystkie pobierania).
    pub async fn models_local_cancel(&self, model_id: Option<String>) -> Result<(), AppError> {
        let rt = self.rt();
        for (id, cancel) in &rt.downloads {
            if model_id.as_ref().is_none_or(|m| m == id) {
                cancel.cancel();
            }
        }
        Ok(())
    }
}
