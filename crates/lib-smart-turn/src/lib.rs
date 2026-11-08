//! Sidecar końca wypowiedzi **Smart Turn v3.2** (pipecat-ai, BSD-2-Clause): model słucha ostatnich
//! 8 s i ocenia, czy rozmówca skończył. ONNX Runtime działa tylko w tym procesie; Alfa rozmawia
//! z nim liniami JSON (`protocol`). Opis i pomiary: `README.md`.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

pub mod engine;
pub mod mel;
pub mod protocol;
