//! `pc-diag` — implementacja (docs/modules/pc-diag/SPEC.md): testy podzespołów
//! ([`compute`], [`disk`], [`network`]), historia ([`history`]) i usługa [`PcDiagService`]
//! (zapis parametrów przez sondę, przebieg testów z postępem, porównanie z poprzednim wynikiem).
//! Testy niczego nie naprawiają ani nie zmieniają w systemie; plik próby dysku jest usuwany
//! zawsze.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

pub mod compute;
pub mod disk;
pub mod history;
pub mod network;
mod service;

pub use service::{
    DISK_LIMIT, DISK_RESERVE_MB, DISK_SAMPLE_MB, OnProgress, PcDiagDeps, PcDiagService, Progress,
};
