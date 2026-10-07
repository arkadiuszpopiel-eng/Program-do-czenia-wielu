//! Kontrakt logów jądra (docs/PLAN.md §13, docs/modules/core-log/SPEC.md).

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

mod redact;
mod sink;

#[cfg(feature = "contract-tests")]
pub mod contract_tests;

pub use redact::{REDACTED, Redactor, RegexRedactor};
pub use sink::{
    AuditRecordRef, AuditWriter, LogError, LogQuery, LogRecord, LogSink, LogStream, RecordRef,
};
