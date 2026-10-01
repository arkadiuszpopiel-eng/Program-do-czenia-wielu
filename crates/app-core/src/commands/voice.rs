//! Komendy `voice_*` ⟶ port `VoicePort` (moduły `voice-*`); lista mikrofonów z `device-profile`,
//! dopóki moduł audio nie jest podłączony.

use device_profile_contract::AudioDirection;
use personas_contract::Personas;

use crate::core::AppCore;
use crate::dto::{AudioDevice, VoiceStatus};
use crate::error::AppError;

impl AppCore {
    /// Mikrofony z detekcji sprzętu (pierwszy = domyślny).
    pub(crate) fn detected_inputs(&self) -> Vec<AudioDevice> {
        self.inner
            .device
            .current()
            .audio
            .unwrap_or_default()
            .into_iter()
            .filter(|d| d.direction == AudioDirection::Input)
            .enumerate()
            .map(|(i, d)| AudioDevice {
                id: format!("mic-{}", i + 1),
                name: d.name,
                default: i == 0,
            })
            .collect()
    }

    /// `voice_devices`.
    pub async fn voice_devices(&self) -> Result<Vec<AudioDevice>, AppError> {
        match self.inner.voice.devices().await? {
            Some(devices) => Ok(devices),
            None => Ok(self.detected_inputs()),
        }
    }

    /// `voice_start_mic_test` (poziomy przez `MicLevel`).
    pub async fn voice_start_mic_test(&self, device_id: Option<String>) -> Result<(), AppError> {
        self.inner.voice.start_mic_test(device_id).await
    }

    /// `voice_stop_mic_test`.
    pub async fn voice_stop_mic_test(&self) -> Result<(), AppError> {
        self.inner.voice.stop_mic_test().await
    }

    /// `voice_set_mic_enabled` (rozmowa głosowa; bez modeli — błąd „głos niedostępny").
    pub async fn voice_set_mic_enabled(&self, enabled: bool) -> Result<(), AppError> {
        let result = self.inner.voice.set_mic_enabled(enabled).await;
        let status = self.inner.voice.status().await;
        self.emit(crate::dto::AlfaEvent::VoiceStatusChanged { status });
        result
    }

    /// `voice_set_muted`.
    pub async fn voice_set_muted(&self, muted: bool) -> Result<(), AppError> {
        self.inner.voice.set_muted(muted).await
    }

    /// `voice_stop_speech`.
    pub async fn voice_stop_speech(&self) -> Result<(), AppError> {
        self.inner.voice.stop_speech().await
    }
}

impl AppCore {
    /// `voice_status`: dostępność trybu głosowego (brak modeli → powód), mikrofon, tryb, agentka.
    pub async fn voice_status(&self) -> Result<VoiceStatus, AppError> {
        Ok(self.inner.voice.status().await)
    }

    /// `voice_ptt`: mówienie z przytrzymaniem (wciśnięcie / puszczenie).
    pub async fn voice_ptt(&self, pressed: bool) -> Result<(), AppError> {
        self.inner.voice.ptt(pressed).await
    }

    /// `voice_preview` ⟶ próbka głosu agentki (głosy v0).
    pub async fn voice_preview(&self, agent: String) -> Result<(), AppError> {
        let known = self
            .inner
            .personas
            .personas()
            .iter()
            .any(|p| p.id.as_str() == agent);
        if !known {
            return Err(AppError::not_found(format!("Nieznana agentka „{agent}”.")));
        }
        self.inner.voice.preview(&agent).await
    }
}
