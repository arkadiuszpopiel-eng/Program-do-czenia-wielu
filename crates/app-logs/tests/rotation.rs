//! Rotacja pliku dziennika: rozmiar, zmiana dnia, limit liczby plików, retencja (7 dni
//! domyślnie, zmiana z konfiguracji), kontynuacja po ponownym otwarciu; pliki innych procesów
//! i segmenty `core-log` nietknięte.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicI64, Ordering};

use app_logs::{LogConfig, LogHandle, Rotation, build, list_files};
use chrono::{DateTime, NaiveDate};

const DAY: i64 = 86_400;
/// 2026-10-06T00:00:00Z.
const T0: i64 = 1_791_244_800;

struct Clock(Arc<AtomicI64>);

impl Clock {
    fn new() -> Self {
        Self(Arc::new(AtomicI64::new(T0)))
    }

    fn advance_days(&self, days: i64) {
        self.0.fetch_add(days * DAY, Ordering::SeqCst);
    }

    fn config(&self, dir: &Path, process: &str, rotation: Rotation) -> LogConfig {
        let secs = self.0.clone();
        LogConfig {
            process: process.into(),
            dir: Some(dir.to_path_buf()),
            filter: "info".into(),
            filter_from_env: false,
            stderr: false,
            rotation,
            clock: Arc::new(move || {
                DateTime::from_timestamp(secs.load(Ordering::SeqCst), 0).unwrap()
            }),
            panic_hook: false,
        }
    }
}

fn day(offset: i64) -> NaiveDate {
    DateTime::from_timestamp(T0 + offset * DAY, 0)
        .unwrap()
        .date_naive()
}

fn emit(config: LogConfig, lines: usize) -> LogHandle {
    let (subscriber, handle) = build(config);
    tracing::subscriber::with_default(subscriber, || {
        for i in 0..lines {
            tracing::info!(target: "app_core::test", n = i, "zdarzenie numer {i:04}");
        }
    });
    handle
}

#[test]
fn size_limit_starts_new_files_and_count_limit_drops_oldest() {
    let dir = tempfile::tempdir().unwrap();
    let clock = Clock::new();
    let rotation = Rotation {
        max_file_bytes: 300,
        max_files: 3,
        retention_days: 7,
    };
    emit(clock.config(dir.path(), "alfa", rotation), 40);
    let files = list_files(dir.path(), "alfa").unwrap();
    assert_eq!(files.len(), 3, "{files:?}");
    assert!(files.iter().all(|f| f.date == day(0)));
    assert!(files[0].seq > 0, "najstarsze usunięte: {files:?}");
    for f in &files {
        let size = std::fs::metadata(&f.path).unwrap().len();
        assert!(size <= 300, "{} ma {size} B", f.path.display());
    }
    let last = std::fs::read_to_string(&files[2].path).unwrap();
    assert!(last.contains("zdarzenie numer 0039"), "{last}");
}

#[test]
fn new_day_new_file_and_retention_removes_old_days() {
    let dir = tempfile::tempdir().unwrap();
    let clock = Clock::new();
    let rotation = Rotation::default();
    std::fs::write(dir.path().join("alfa-broker.2026-09-01.000.log"), b"obcy\n").unwrap();
    std::fs::create_dir_all(dir.path().join("diagnostics")).unwrap();
    std::fs::write(dir.path().join("00000000000000000000.ndjson"), b"{}\n").unwrap();
    emit(clock.config(dir.path(), "alfa", rotation), 2);
    clock.advance_days(1);
    emit(clock.config(dir.path(), "alfa", rotation), 2);
    let dates: Vec<_> = list_files(dir.path(), "alfa")
        .unwrap()
        .iter()
        .map(|f| f.date)
        .collect();
    assert_eq!(dates, [day(0), day(1)]);

    clock.advance_days(8);
    emit(clock.config(dir.path(), "alfa", rotation), 1);
    let dates: Vec<_> = list_files(dir.path(), "alfa")
        .unwrap()
        .iter()
        .map(|f| f.date)
        .collect();
    assert_eq!(dates, [day(9)], "pliki starsze niż 7 dni usunięte");
    assert!(dir.path().join("alfa-broker.2026-09-01.000.log").is_file());
    assert!(dir.path().join("00000000000000000000.ndjson").is_file());
    assert!(dir.path().join("diagnostics").is_dir());
}

#[test]
fn reopening_continues_todays_file() {
    let dir = tempfile::tempdir().unwrap();
    let clock = Clock::new();
    let rotation = Rotation::default();
    emit(clock.config(dir.path(), "alfa", rotation), 2);
    emit(clock.config(dir.path(), "alfa", rotation), 3);
    let files = list_files(dir.path(), "alfa").unwrap();
    assert_eq!(files.len(), 1);
    let text = std::fs::read_to_string(&files[0].path).unwrap();
    assert_eq!(text.lines().count(), 5);
}

#[test]
fn retention_from_settings_prunes_immediately_and_is_clamped() {
    let dir = tempfile::tempdir().unwrap();
    let clock = Clock::new();
    let rotation = Rotation::default();
    for _ in 0..5 {
        emit(clock.config(dir.path(), "alfa", rotation), 1);
        clock.advance_days(1);
    }
    assert_eq!(list_files(dir.path(), "alfa").unwrap().len(), 5);
    let handle = emit(clock.config(dir.path(), "alfa", rotation), 1);
    assert_eq!(list_files(dir.path(), "alfa").unwrap().len(), 6);
    assert!(handle.apply_settings(None, Some(2)).is_empty());
    let dates: Vec<_> = list_files(dir.path(), "alfa")
        .unwrap()
        .iter()
        .map(|f| f.date)
        .collect();
    assert_eq!(dates, [day(3), day(4), day(5)]);
    assert!(handle.apply_settings(None, Some(0)).is_empty());
    assert_eq!(
        list_files(dir.path(), "alfa").unwrap().len(),
        2,
        "0 dni przycięte do 1"
    );
    assert_eq!(handle.dir().unwrap(), dir.path());
}
