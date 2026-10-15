//! Komendy `cameras_*`: lista kamer, przełączniki, nazwy i klatka na żądanie (`camera-hub`).
//! Klatka wraca do interfejsu jako PNG w base64 — nie trafia na dysk, do logów ani do zdarzeń.
//! Przekład stanu na DTO, błędy i kodowanie PNG: `app_modules::cameras`.

use std::sync::atomic::Ordering;

use app_modules::cameras::{camera_error, cameras_dto, png_base64, zlib_fast};
use camera_hub_contract::CameraId;
use platform_contract::encode_png_with;

use crate::core::AppCore;
use crate::dto::{
    AttachmentsAdded, CAMERA_FRAME_MAX_SIDE, CAMERA_FRAME_MIN_SIDE, CameraFrame, CamerasState,
};
use crate::error::AppError;

impl AppCore {
    /// `cameras_state`: przełącznik główny i lista kamer. Pierwsze wywołanie wylicza urządzenia
    /// systemu (bez otwierania kamer); kolejne zwracają stan z pamięci.
    pub async fn cameras_state(&self) -> Result<CamerasState, AppError> {
        let cameras = &self.inner.cameras;
        if !cameras.listed.swap(true, Ordering::SeqCst)
            && let Err(e) = cameras.hub.refresh().await
        {
            tracing::warn!(error = %e, "nie udało się wyliczyć kamer");
        }
        Ok(cameras_dto(&cameras.hub.state().await))
    }

    /// `cameras_set_master`: przełącznik główny (wyłączenie zamyka wszystkie kamery od razu).
    pub async fn cameras_set_master(&self, enabled: bool) -> Result<CamerasState, AppError> {
        let hub = &self.inner.cameras.hub;
        let state = hub.set_master(enabled).await.map_err(camera_error)?;
        Ok(cameras_dto(&state))
    }

    /// `cameras_set_enabled`: przełącznik jednej kamery.
    pub async fn cameras_set_enabled(
        &self,
        id: String,
        enabled: bool,
    ) -> Result<CamerasState, AppError> {
        let hub = &self.inner.cameras.hub;
        let state = hub.set_enabled(&CameraId(id), enabled).await;
        Ok(cameras_dto(&state.map_err(camera_error)?))
    }

    /// `cameras_rename`: nazwa kamery (pusta przywraca nazwę systemową).
    pub async fn cameras_rename(&self, id: String, name: String) -> Result<CamerasState, AppError> {
        let hub = &self.inner.cameras.hub;
        let state = hub.rename(&CameraId(id), &name).await;
        Ok(cameras_dto(&state.map_err(camera_error)?))
    }

    /// `cameras_refresh`: ponowne wyliczenie urządzeń (podłączone i odłączone kamery).
    pub async fn cameras_refresh(&self) -> Result<CamerasState, AppError> {
        let cameras = &self.inner.cameras;
        cameras.listed.store(true, Ordering::SeqCst);
        let state = cameras.hub.refresh().await.map_err(camera_error)?;
        Ok(cameras_dto(&state))
    }

    /// `cameras_frame`: jedna bieżąca klatka jako PNG; `max_side` przycinane do 64–1280.
    /// Budzi kamerę, jeśli śpi; odmawia przy wyłączonym przełączniku głównym albo kamery.
    pub async fn cameras_frame(&self, id: String, max_side: u32) -> Result<CameraFrame, AppError> {
        let side = max_side.clamp(CAMERA_FRAME_MIN_SIDE, CAMERA_FRAME_MAX_SIDE);
        let hub = &self.inner.cameras.hub;
        let frame = hub.frame(&CameraId(id), side).await.map_err(camera_error)?;
        let (camera, taken_ms) = (frame.camera.0, frame.taken_ms);
        let image = frame.image;
        tokio::task::spawn_blocking(move || CameraFrame {
            camera,
            width: image.width,
            height: image.height,
            taken_ms,
            png_base64: png_base64(&image),
        })
        .await
        .map_err(|e| AppError::internal(format!("kodowanie klatki: {e}")))
    }

    /// `attachments_add_camera`: jedna klatka z kamery (dłuższy bok ≤ 1280) jako załącznik PNG
    /// composera — ta sama ścieżka co obraz wklejony ze schowka. To jedyne miejsce, w którym
    /// klatka trafia na dysk, i tylko na wyraźne polecenie użytkownika.
    pub async fn attachments_add_camera(
        &self,
        session_id: String,
        camera_id: String,
    ) -> Result<AttachmentsAdded, AppError> {
        let session = crate::ids::session(&session_id)?;
        self.ensure_session(&session)?;
        let hub = &self.inner.cameras.hub;
        let frame = hub
            .frame(&CameraId(camera_id), CAMERA_FRAME_MAX_SIDE)
            .await
            .map_err(camera_error)?;
        let files = self.inner.work.files.clone();
        tokio::task::spawn_blocking(move || {
            let png = encode_png_with(&frame.image, &zlib_fast);
            files.attachments().add_png(&session, &png)
        })
        .await
        .map_err(|e| AppError::internal(format!("kodowanie klatki: {e}")))?
    }
}
