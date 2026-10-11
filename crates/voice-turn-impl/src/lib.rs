//! Implementacja modułu `voice-turn` (docs/modules/voice-turn/SPEC.md): `PatienceTurnDetector`
//! (cierpliwość: minimalna cisza, cisza bazowa, wydłużenie po hezytacji i przy niepewnym modelu,
//! twardy limit) oraz `HeuristicTurnModel` (model tekstowy) i `SmartTurnModel`
//! (Smart Turn v3.2 na dźwięku — liczy osobny proces `alfa-smart-turn`). Całość deterministyczna — czas podaje wywołujący (wirtualny zegar w testach).

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

mod detector;
mod hesitation;
mod model;
mod smart_turn;
mod turn_sidecar;

pub use detector::PatienceTurnDetector;
pub use hesitation::{Hesitation, ends_clearly, hesitation};
pub use model::{HeuristicTurnModel, P_CLEAR, P_HESITATION, P_NEUTRAL, P_QUESTION};
pub use smart_turn::{
    AudioScorer, MODEL_FILE, REPROBE_MS, SAMPLE_RATE, SmartTurnModel, WINDOW_SAMPLES,
};
pub use turn_sidecar::{SidecarCfg, SidecarScorer, encode_tail, parse_reply};

/// Treść `module.toml` tego modułu.
pub const MODULE_TOML: &str = include_str!("../module.toml");
