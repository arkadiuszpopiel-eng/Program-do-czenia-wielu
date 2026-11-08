//! Sidecar mowy Supertonic 3: osobny proces `alfa-supertonic`, który `voice-tts-impl` uruchamia
//! i z którym rozmawia liniami JSON po stdio. ONNX Runtime działa tylko w tym procesie (ADR 0004).
//!
//! Wagi modelu mają licencję OpenRAIL-M (lista zakazanych zastosowań — `docs/plan/15-glos-lokalny.md`);
//! kod przygotowania tekstu i przebiegu syntezy powstał na podstawie przykładu producenta (MIT).

pub mod engine;
pub mod protocol;
pub mod text;
