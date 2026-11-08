//! Komendy `device_*`: profil sprzętu (`device-profile`) w kształcie UI; przekształcenia
//! w `app_modules::device_view`.

use app_modules::device_view::{device_dto, gpu_live_view};

use crate::core::AppCore;
use crate::dto::{DeviceProfile, GpuLive};
use crate::error::AppError;

impl AppCore {
    fn device_now(&self) -> DeviceProfile {
        let device = &self.inner.device;
        let name = std::env::var("COMPUTERNAME").unwrap_or_else(|_| "Ten komputer".to_owned());
        device_dto(&device.current(), &device.recommend(), name)
    }

    /// `device_profile`.
    pub async fn device_profile(&self) -> Result<DeviceProfile, AppError> {
        Ok(self.device_now())
    }

    /// `device_measure`: ponowna detekcja sprzętu.
    pub async fn device_measure(&self) -> Result<DeviceProfile, AppError> {
        if let Err(e) = self.inner.device.refresh() {
            tracing::warn!(error = %e, "ponowna detekcja sprzętu nie powiodła się");
        }
        Ok(self.device_now())
    }

    /// `device_gpu_live`: karty graficzne na żywo (pamięć, obciążenie, temperatura, moc, programy).
    /// Odczyt uruchamia narzędzie sterownika, więc idzie poza wątkiem obsługi poleceń.
    pub async fn device_gpu_live(&self) -> Result<GpuLive, AppError> {
        let read = tokio::task::spawn_blocking(app_modules::gpu_mem::live)
            .await
            .map_err(|e| AppError::internal(e.to_string()))?;
        Ok(read.map_or_else(GpuLive::default, gpu_live_view))
    }
}
