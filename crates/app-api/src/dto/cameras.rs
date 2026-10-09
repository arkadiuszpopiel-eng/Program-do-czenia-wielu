//! DTO komend `cameras_*` i zdarzenia `CamerasChanged` (`camera-hub`, plan v6 grupa C3) —
//! odpowiednik `types-cameras.ts`.

use serde::{Deserialize, Serialize};

/// Najmniejszy `max_side` klatki dla interfejsu.
pub const CAMERA_FRAME_MIN_SIDE: u32 = 64;
/// Największy `max_side` klatki dla interfejsu.
pub const CAMERA_FRAME_MAX_SIDE: u32 = 1280;

/// Rodzaj kamery.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CameraKindView {
    Usb,
    Network,
}

/// Stan strumienia kamery.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CameraStateView {
    Asleep,
    Opening,
    Live,
    Failed,
    Unplugged,
}

/// Kamera na liście.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CameraView {
    pub id: String,
    pub name: String,
    pub kind: CameraKindView,
    pub enabled: bool,
    pub state: CameraStateView,
    /// Opis błędu przy stanie `failed`.
    pub detail: Option<String>,
    /// Rozmiar obrazu kamery; 0, dopóki kamera nie działa.
    pub width: u32,
    pub height: u32,
}

/// Stan kamer: przełącznik główny i lista.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct CamerasState {
    pub enabled: bool,
    pub cameras: Vec<CameraView>,
}

/// Klatka z kamery jako PNG (base64), już pomniejszona.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CameraFrame {
    pub camera: String,
    pub width: u32,
    pub height: u32,
    /// Chwila zrobienia klatki (ms od 1970).
    pub taken_ms: u64,
    pub png_base64: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wire_shape_is_snake_case_with_lowercase_enums() {
        let state = CamerasState {
            enabled: true,
            cameras: vec![CameraView {
                id: "cam".into(),
                name: "Kuchnia".into(),
                kind: CameraKindView::Usb,
                enabled: true,
                state: CameraStateView::Unplugged,
                detail: None,
                width: 0,
                height: 0,
            }],
        };
        let json = serde_json::to_value(&state).unwrap();
        assert_eq!(json["cameras"][0]["kind"], "usb");
        assert_eq!(json["cameras"][0]["state"], "unplugged");
        assert!(json["cameras"][0]["detail"].is_null());
        assert_eq!(serde_json::from_value::<CamerasState>(json).unwrap(), state);
        let frame = CameraFrame {
            camera: "cam".into(),
            width: 2,
            height: 1,
            taken_ms: 7,
            png_base64: "AA==".into(),
        };
        let json = serde_json::to_value(&frame).unwrap();
        assert_eq!(json["taken_ms"], 7);
        assert_eq!(json["png_base64"], "AA==");
        assert_eq!((CAMERA_FRAME_MIN_SIDE, CAMERA_FRAME_MAX_SIDE), (64, 1280));
    }
}
