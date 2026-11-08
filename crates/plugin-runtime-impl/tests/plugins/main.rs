//! Testy `plugin-runtime-impl` (jeden plik binarny — każdy linkuje wasmtime): wtyczki-
//! napastniczki z WAT (THREAT_MODEL S09, ACCEPTANCE F8-05: ≥ 50 przypadków, 0 ucieczek),
//! cykl życia (kontrakt), licznik słów w rejestrze narzędzi i pętli modelu na atrapach.

#![allow(dead_code, clippy::unwrap_used, clippy::expect_used)]

mod common;
mod exec;
mod hostcalls;
mod hostok;
mod life;
mod load;
mod supply;
