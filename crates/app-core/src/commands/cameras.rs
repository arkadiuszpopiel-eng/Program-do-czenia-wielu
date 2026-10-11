//! Komendy `cameras_*`: lista kamer, przełączniki, nazwy i klatka na żądanie (`camera-hub`).
//! Klatka wraca do interfejsu jako PNG w base64 — nie trafia na dysk, do logów ani do zdarzeń.

use std::io::Write;
use std::sync::atomic::Ordering;

use base64::Engine;
use camera_hub_contract::{
    CameraError, CameraId, CameraInfo, CameraKind, CameraState, HubState, RgbaImage,
};
use flate2::Compression;
use flate2::write::ZlibEncoder;
use platform_contract::{encode_png_with, zlib_stored};

use crate::core::AppCore;
use crate::dto::{
    AttachmentsAdded, CAMERA_FRAME_MAX_SIDE, CAMERA_FRAME_MIN_SIDE, CameraFrame, CameraKindView,
    CameraStateView, CameraView, CamerasState,
};
use crate::error::{AppError, ErrorCode};

fn camera_view(camera: &CameraInfo) -> CameraView {
    let (state, detail, width, height) = match &camera.state {
        CameraState::Asleep => (CameraStateView::Asleep, None, 0, 0),
        CameraState::Opening => (CameraStateView::Opening, None, 0, 0),
        CameraState::Live { width, height } => (CameraStateView::Live, None, *width, *height),
        CameraState::Closing => (CameraStateView::Closing, None, 0, 0),
        CameraState::Failed(text) => (CameraStateView::Failed, Some(text.clone()), 0, 0),
        CameraState::Unplugged => (CameraStateView::Unplugged, None, 0, 0),
    };
    CameraView {
        id: camera.id.0.clone(),
        name: camera.name.clone(),
        kind: match camera.kind {
            CameraKind::Usb => CameraKindView::Usb,
            CameraKind::Network => CameraKindView::Network,
        },
        enabled: camera.enabled,
        state,
        detail,
        width,
        height,
    }
}

/// Stan huba → DTO.
pub(crate) fn cameras_dto(state: &HubState) -> CamerasState {
    CamerasState {
        enabled: state.enabled,
        cameras: state.cameras.iter().map(camera_view).collect(),
    }
}

fn camera_error(error: CameraError) -> AppError {
    match error {
        CameraError::Disabled => AppError::forbidden(
            "Kamery są wyłączone — włącz przełącznik kamer i przełącznik tej kamery.",
        )
        .with("reason", "disabled"),
        CameraError::UnknownCamera(id) => AppError::missing("camera", id, "Nie ma takiej kamery."),
        CameraError::Store(text) => AppError::storage(text),
        CameraError::Unplugged => unavailable("unplugged", "Kamera jest odłączona."),
        CameraError::Stopped => unavailable("stopped", "Kamera została zatrzymana."),
        CameraError::Busy => unavailable("busy", "Kamera właśnie się otwiera — spróbuj za chwilę."),
        CameraError::Timeout => {
            unavailable("timeout", "Kamera nie odpowiedziała w wyznaczonym czasie.")
        }
        CameraError::Platform(text) => {
            unavailable("platform", format!("Kamera nie działa: {text}"))
        }
    }
}

/// Kamera chwilowo niedostępna; `reason` pozwala interfejsowi rozróżnić przypadki bez czytania tekstu.
fn unavailable(reason: &str, message: impl Into<String>) -> AppError {
    AppError::new(ErrorCode::Unavailable, message)
        .with("kind", "camera")
        .with("reason", reason)
}

/// Kompresja zlib dla PNG klatki (poziom szybki — podgląd pobiera klatki kilka razy na sekundę).
fn zlib_fast(data: &[u8]) -> Vec<u8> {
    let mut encoder = ZlibEncoder::new(Vec::with_capacity(data.len() / 3), Compression::fast());
    if encoder.write_all(data).is_err() {
        return zlib_stored(data);
    }
    encoder.finish().unwrap_or_else(|_| zlib_stored(data))
}

fn png_base64(image: &RgbaImage) -> String {
    base64::engine::general_purpose::STANDARD.encode(encode_png_with(image, &zlib_fast))
}

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
