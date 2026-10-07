//! Implementacja modułu `voice-turn` (docs/modules/voice-turn/SPEC.md): `PatienceTurnDetector`
//! (cierpliwość: minimalna cisza, cisza bazowa, wydłużenie po hezytacji i przy niepewnym modelu,
//! twardy limit) oraz `HeuristicTurnModel` (model tekstowy zastępujący Smart Turn v3.2 do czasu
//! integracji ONNX). Całość deterministyczna — czas podaje wywołujący (wirtualny zegar w testach).

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

mod detector;
mod hesitation;
mod model;

pub use detector::PatienceTurnDetector;
pub use hesitation::{Hesitation, ends_clearly, hesitation};
pub use model::{HeuristicTurnModel, P_CLEAR, P_HESITATION, P_NEUTRAL, P_QUESTION};

/// Treść `module.toml` tego modułu.
pub const MODULE_TOML: &str = include_str!("../module.toml");
