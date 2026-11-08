//! Log-writer jądra (docs/PLAN.md §13, docs/modules/core-log/SPEC.md).
//!
//! `FileLogSink` implementuje `LogSink`: append-only NDJSON per strumień w segmentach
//! (rotacja po rozmiarze), limit dysku per strumień (usuwanie najstarszych segmentów), retencja
//! w dniach (Narzędzia/GUI domyślnie 7), redakcja `Redactor` przed zapisem, `query` skanem plików.
//! `PreBrokerAuditWriter` implementuje `AuditWriter` do czasu przejęcia Audytu przez Broker (F3):
//! łańcuch SHA-256 po kanonicznym JSON, `verify_chain` wykrywa modyfikację/usunięcie/wstawienie.
//! `spawn_bus_writer` zapisuje zdarzenia z magistrali.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

mod audit;
mod bus_writer;
mod canonical;
mod options;
mod segment;
mod sink;

pub use audit::{
    AuditOpenError, ChainError, ChainSummary, PRE_BROKER_WRITER, PreBrokerAuditWriter,
    verify_against, verify_bytes, verify_file,
};
pub use bus_writer::{Route, route, spawn_bus_writer};
pub use canonical::{canonical_json, sha256_hex};
pub use options::{
    Clock, DEFAULT_SEGMENT_BYTES, DEFAULT_STREAM_DISK_LIMIT, DEFAULT_TOOLS_GUI_RETENTION_DAYS,
    LogOptions, StreamLimits, SystemClock,
};
pub use segment::{DiskRecord, stream_dir_name};
pub use sink::FileLogSink;

/// Treść `module.toml` log-writera (walidowana testem przez `ModuleManifest::parse_toml`).
pub const MODULE_TOML: &str = include_str!("../module.toml");
