//! Komendy `system_*`: stan systemu (PLAN §14.4) i ponowienie kolejki offline.

use crate::core::AppCore;
use crate::dto::{
    AlfaEvent, DiskInfo, LocalizedText, MicAvailability, RateLimitInfo, SystemStatus, ToastKind,
};
use crate::error::AppError;

/// Próg „mało miejsca" (2 GB).
const DISK_LOW_BYTES: u64 = 2 * 1024 * 1024 * 1024;

impl AppCore {
    /// Bieżący stan systemu (dla zdarzeń `SystemStatusChanged`).
    pub(crate) async fn status_snapshot(&self) -> SystemStatus {
        let now = chrono::Utc::now();
        let (online, rate_limit) = {
            let mut rt = self.rt();
            if rt.rate_limit.as_ref().is_some_and(|(_, at)| *at <= now) {
                rt.rate_limit = None;
            }
            (rt.online, rt.rate_limit.clone())
        };
        let free = self.inner.shell.disk_free(&self.inner.paths.local);
        SystemStatus {
            online,
            queued_messages: self.queued_count(),
            rate_limit: rate_limit.map(|(provider, at)| RateLimitInfo {
                provider,
                resets_at: crate::dto::iso(at),
            }),
            keys_configured: self.inner.brain.keys_configured(),
            profile: self.default_profile().await,
            mic: if self.detected_inputs().is_empty() && self.inner.device.current().audio.is_some()
            {
                MicAvailability::Missing
            } else {
                MicAvailability::Ok
            },
            disk: DiskInfo {
                free_bytes: free.unwrap_or(0),
                low: free.is_some_and(|f| f < DISK_LOW_BYTES),
            },
        }
    }

    /// `system_status`: online, kolejka, 429, klucze, mikrofon, dysk.
    pub async fn system_status(&self) -> Result<SystemStatus, AppError> {
        Ok(self.status_snapshot().await)
    }

    /// `system_retry_queue`: offline → komunikat; online → wysłanie kolejki.
    pub async fn system_retry_queue(&self) -> Result<(), AppError> {
        if !self.rt().online {
            self.emit(AlfaEvent::Toast {
                kind: ToastKind::Warning,
                message: LocalizedText::new("Nadal brak połączenia.", "Still offline."),
            });
            return Ok(());
        }
        self.flush_queue().await?;
        let status = self.status_snapshot().await;
        self.emit(AlfaEvent::SystemStatusChanged { status });
        Ok(())
    }
}
