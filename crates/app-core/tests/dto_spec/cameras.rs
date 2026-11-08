//! Specyfikacje komend kamer (`cameras_*`, `attachments_add_camera`) i wznowienia zdarzeń
//! (`events_resume`) dla `dto_roundtrip.rs`.

use app_core::dto::*;

use crate::{Check, roundtrip};

use super::Spec;

/// Specyfikacja komendy kamer (`None` — komenda spoza tej części).
pub fn spec(command: &str) -> Option<Spec> {
    let state: Check = roundtrip::<CamerasState>;
    let id: (&str, Check) = ("id", roundtrip::<String>);
    Some(match command {
        "cameras_state" | "cameras_refresh" => (vec![], state),
        "cameras_set_master" => (vec![("enabled", roundtrip::<bool>)], state),
        "cameras_set_enabled" => (vec![id, ("enabled", roundtrip::<bool>)], state),
        "cameras_rename" => (vec![id, ("name", roundtrip::<String>)], state),
        "cameras_frame" => (
            vec![id, ("maxSide", roundtrip::<u32>)],
            roundtrip::<CameraFrame>,
        ),
        "attachments_add_camera" => (
            vec![
                ("sessionId", roundtrip::<String>),
                ("cameraId", roundtrip::<String>),
            ],
            roundtrip::<AttachmentsAdded>,
        ),
        "events_resume" => (
            vec![
                ("afterSeq", roundtrip::<u64>),
                ("epoch", roundtrip::<Option<String>>),
            ],
            roundtrip::<EventsResume>,
        ),
        _ => return None,
    })
}
