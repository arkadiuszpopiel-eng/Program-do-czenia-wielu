//! `Subscriber` dziennika: filtr poziomów, zbieranie pól z redakcją, linia tekstu do pliku
//! (i na stderr w buildzie debug). Spany nie są śledzone (Alfa ich nie używa) — dostają tylko
//! identyfikator, kontekst spanów nie trafia do pliku.

use std::fmt;
use std::io::Write;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, MutexGuard, PoisonError, RwLock};

use chrono::{DateTime, Utc};
use tracing::field::{Field, Visit};
use tracing::level_filters::LevelFilter;
use tracing::span::{Attributes, Id, Record};
use tracing::subscriber::Interest;
use tracing::{Event, Level, Metadata, Subscriber};

use crate::file::{Clock, RollingFile};
use crate::filter::Filter;
use crate::redact::{MAX_FIELDS, Redaction, escape};

/// Stan wspólny subskrybenta i uchwytu.
pub(crate) struct Shared {
    pub filter: RwLock<Filter>,
    pub file: Mutex<Option<RollingFile>>,
    pub stderr: bool,
    pub redaction: Redaction,
    pub clock: Clock,
    pub next_span: AtomicU64,
    pub problem: Mutex<Option<String>>,
    pub failed_writes: AtomicU64,
    pub filter_from_env: bool,
}

pub(crate) fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

impl Shared {
    pub fn filter(&self) -> Filter {
        self.filter
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    fn enabled(&self, meta: &Metadata<'_>) -> bool {
        self.filter
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .enabled(meta.target(), meta.level())
    }

    pub fn set_problem(&self, problem: String) {
        *lock(&self.problem) = Some(problem);
    }

    fn write_event(&self, event: &Event<'_>) {
        let meta = event.metadata();
        let mut collector = Collector {
            redaction: &self.redaction,
            message: None,
            fields: Vec::new(),
            skipped: 0,
        };
        event.record(&mut collector);
        let line = collector.line((self.clock)(), meta.level(), meta.target());
        if self.stderr {
            let _ = std::io::stderr().lock().write_all(line.as_bytes());
        }
        let written = match lock(&self.file).as_mut() {
            Some(file) => file.write_line(line.as_bytes()),
            None => Ok(()),
        };
        if let Err(e) = written {
            // Bez `tracing` tutaj (rekurencja) — błąd widać w `LogHandle::problem`.
            self.failed_writes.fetch_add(1, Ordering::Relaxed);
            self.set_problem(format!("zapis dziennika: {e}"));
        }
    }
}

/// Pola jednego zdarzenia po redakcji.
struct Collector<'a> {
    redaction: &'a Redaction,
    message: Option<String>,
    fields: Vec<(&'static str, String)>,
    skipped: usize,
}

impl Collector<'_> {
    fn push(&mut self, field: &Field, raw: &str, is_bool: bool) {
        if field.name() == "message" {
            self.message = Some(self.redaction.message(raw));
            return;
        }
        if self.fields.len() >= MAX_FIELDS {
            self.skipped += 1;
            return;
        }
        let value = self.redaction.field(field.name(), raw, is_bool);
        self.fields.push((field.name(), value));
    }

    fn line(self, ts: DateTime<Utc>, level: &Level, target: &str) -> String {
        let mut line = format!(
            "{} {:>5} {}:",
            ts.format("%Y-%m-%dT%H:%M:%S%.3fZ"),
            level.as_str(),
            escape(target)
        );
        if let Some(message) = self.message {
            line.push(' ');
            line.push_str(&message);
        }
        for (name, value) in self.fields {
            line.push(' ');
            line.push_str(&escape(name));
            line.push('=');
            if value.is_empty() || value.contains([' ', '"', '=']) {
                line.push('"');
                line.push_str(&value.replace('"', "\\\""));
                line.push('"');
            } else {
                line.push_str(&value);
            }
        }
        if self.skipped > 0 {
            line.push_str(&format!(" [+{} pól]", self.skipped));
        }
        line.push('\n');
        line
    }
}

impl Visit for Collector<'_> {
    fn record_debug(&mut self, field: &Field, value: &dyn fmt::Debug) {
        self.push(field, &format!("{value:?}"), false);
    }

    fn record_str(&mut self, field: &Field, value: &str) {
        self.push(field, value, false);
    }

    fn record_bool(&mut self, field: &Field, value: bool) {
        self.push(field, if value { "true" } else { "false" }, true);
    }

    fn record_i64(&mut self, field: &Field, value: i64) {
        self.push(field, &value.to_string(), false);
    }

    fn record_u64(&mut self, field: &Field, value: u64) {
        self.push(field, &value.to_string(), false);
    }

    fn record_f64(&mut self, field: &Field, value: f64) {
        self.push(field, &value.to_string(), false);
    }

    fn record_error(&mut self, field: &Field, value: &(dyn std::error::Error + 'static)) {
        self.push(field, &value.to_string(), false);
    }
}

/// Subskrybent dziennika (instaluje go `crate::install`; w testach — `with_default`).
pub struct FileSubscriber(pub(crate) std::sync::Arc<Shared>);

impl Subscriber for FileSubscriber {
    fn register_callsite(&self, meta: &'static Metadata<'static>) -> Interest {
        if self.0.enabled(meta) {
            Interest::always()
        } else {
            Interest::never()
        }
    }

    fn enabled(&self, meta: &Metadata<'_>) -> bool {
        self.0.enabled(meta)
    }

    fn max_level_hint(&self) -> Option<LevelFilter> {
        Some(self.0.filter().max_level())
    }

    fn new_span(&self, _span: &Attributes<'_>) -> Id {
        Id::from_u64(self.0.next_span.fetch_add(1, Ordering::Relaxed).max(1))
    }

    fn record(&self, _span: &Id, _values: &Record<'_>) {}

    fn record_follows_from(&self, _span: &Id, _follows: &Id) {}

    fn event(&self, event: &Event<'_>) {
        if self.0.enabled(event.metadata()) {
            self.0.write_event(event);
        }
    }

    fn enter(&self, _span: &Id) {}

    fn exit(&self, _span: &Id) {}
}
