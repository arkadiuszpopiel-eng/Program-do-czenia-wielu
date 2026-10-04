//! Testy integracyjne `lib-embed` w jednym binarium (każde linkuje `tract` — oszczędność dysku):
//! tokenizer vs HF `tokenizers` (wektory referencyjne), model zabawkowy vs onnxruntime, embedder
//! (wątek tła, prefiksy, dzierżawa, bezczynność), instalator (wznawianie, hashe), generator referencji.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod dump;
mod install;
mod model;
mod residency;
mod tokenizer;
mod xlmr;
