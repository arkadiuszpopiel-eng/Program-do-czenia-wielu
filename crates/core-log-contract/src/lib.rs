//! Kontrakt logów jądra (docs/PLAN.md §13, docs/modules/core-log/SPEC.md).

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

mod redact;
mod sink;

pub use redact::{Redactor, RegexRedactor, REDACTED};
pub use sink::{
    AuditRecordRef, AuditWriter, LogError, LogQuery, LogRecord, LogSink, LogStream, RecordRef,
};
