//! `FileLogSink` — append-only NDJSON per strumień z rotacją, limitem dysku i retencją.

use std::collections::BTreeMap;
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard};

use async_trait::async_trait;
use chrono::{DateTime, Duration, Utc};
use core_bus_contract::{EVENT_SCHEMA_VERSION, Event};
use core_log_contract::{
    LogError, LogQuery, LogRecord, LogSink, LogStream, RecordRef, Redactor, RegexRedactor,
};

use crate::options::{Clock, LogOptions, SystemClock};
use crate::segment::{
    DiskRecord, Segment, ends_cleanly, io_err, list_segments, read_records, segment_path,
    stream_dir_name,
};

/// Stan jednego strumienia.
struct StreamState {
    dir: PathBuf,
    segments: Vec<Segment>,
    next_seq: u64,
    active: Option<File>,
    tail_broken: bool,
}

impl StreamState {
    fn open(dir: PathBuf) -> Result<Self, LogError> {
        fs::create_dir_all(&dir).map_err(|e| io_err("tworzenie katalogu logów", e))?;
        let mut segments = list_segments(&dir)?;
        let mut state = (0, false);
        if let Some(last) = segments.last() {
            let records = read_records(&last.path)?;
            let torn = !ends_cleanly(&last.path)?;
            state = (records.last().map_or(last.first_seq, |r| r.seq + 1), torn);
            if torn && records.is_empty() {
                // Sam urwany zapis: odkładamy plik jako `.broken` (dowód), numer segmentu wraca do użytku.
                let mut quarantined = last.path.clone().into_os_string();
                quarantined.push(".broken");
                fs::rename(&last.path, &quarantined)
                    .map_err(|e| io_err("kwarantanna segmentu", e))?;
                segments.pop();
                state.1 = false;
            }
        }
        let (next_seq, tail_broken) = state;
        Ok(Self {
            dir,
            segments,
            next_seq,
            active: None,
            tail_broken,
        })
    }

    fn total_bytes(&self) -> u64 {
        self.segments.iter().map(|s| s.bytes).sum()
    }

    /// Nowy segment od `next_seq` (plik tworzony, nigdy nadpisywany).
    fn rotate(&mut self) -> Result<(), LogError> {
        let path = segment_path(&self.dir, self.next_seq);
        let file = OpenOptions::new()
            .create_new(true)
            .append(true)
            .open(&path)
            .map_err(|e| io_err("tworzenie segmentu", e))?;
        self.segments.push(Segment {
            first_seq: self.next_seq,
            path,
            bytes: 0,
            last_written: None,
        });
        self.active = Some(file);
        self.tail_broken = false;
        Ok(())
    }

    fn active_file(&mut self) -> Result<&mut File, LogError> {
        if self.active.is_none() {
            let path = self
                .segments
                .last()
                .map(|s| s.path.clone())
                .ok_or_else(|| LogError::Io("brak aktywnego segmentu".into()))?;
            let file = OpenOptions::new()
                .append(true)
                .open(&path)
                .map_err(|e| io_err("otwarcie segmentu", e))?;
            self.active = Some(file);
        }
        self.active
            .as_mut()
            .ok_or_else(|| LogError::Io("brak aktywnego segmentu".into()))
    }

    fn remove_oldest(&mut self) -> Result<(), LogError> {
        if self.segments.len() == 1 {
            self.active = None;
        }
        if !self.segments.is_empty() {
            let old = self.segments.remove(0);
            fs::remove_file(&old.path).map_err(|e| io_err("usuwanie segmentu", e))?;
        }
        Ok(())
    }

    /// Usuwa segmenty, których ostatni rekord jest starszy niż `cutoff`; zwraca ich liczbę.
    /// Pusty segment jest wygasły tylko wtedy, gdy nie jest ostatni. Gdy znikną wszystkie,
    /// powstaje pusty segment od `next_seq` — numeracja przetrwa restart.
    fn enforce_retention(&mut self, cutoff: DateTime<Utc>) -> Result<usize, LogError> {
        let mut removed = 0;
        loop {
            let is_last = self.segments.len() == 1;
            let Some(first) = self.segments.first_mut() else {
                break;
            };
            if first.last_written.is_none() {
                first.last_written = read_records(&first.path)?.last().map(|r| r.written_at);
            }
            let expired = match first.last_written {
                Some(t) => t < cutoff,
                None => !is_last,
            };
            if !expired {
                break;
            }
            self.remove_oldest()?;
            removed += 1;
        }
        if removed > 0 && self.segments.is_empty() {
            self.rotate()?;
        }
        Ok(removed)
    }

    /// Czy dopisanie `len` bajtów wymaga nowego segmentu.
    fn needs_rotation(&self, len: u64, max_segment: u64) -> bool {
        match self.segments.last() {
            None => true,
            Some(last) => self.tail_broken || (last.bytes > 0 && last.bytes + len > max_segment),
        }
    }
}

/// Log-writer strumieni jądra (bez Audytu). Zapis synchroniczny pod zamkiem, bez fsync
/// per rekord (SPEC: grupowanie); kolejka i osobny wątek I/O — w F1.
pub struct FileLogSink {
    options: LogOptions,
    redactor: Arc<dyn Redactor>,
    clock: Arc<dyn Clock>,
    streams: Mutex<BTreeMap<LogStream, StreamState>>,
}

impl FileLogSink {
    /// Otwiera (lub tworzy) katalogi strumieni, odtwarza numerację i stosuje retencję.
    /// Redaktor domyślny: `RegexRedactor::default()`, zegar systemowy.
    pub fn open(options: LogOptions) -> Result<Self, LogError> {
        Self::open_with(
            options,
            Arc::new(RegexRedactor::default()),
            Arc::new(SystemClock),
        )
    }

    /// Jak `open`, z własnym redaktorem i zegarem.
    pub fn open_with(
        options: LogOptions,
        redactor: Arc<dyn Redactor>,
        clock: Arc<dyn Clock>,
    ) -> Result<Self, LogError> {
        let mut streams = BTreeMap::new();
        for stream in LogStream::ALL {
            let dir = options.root.join(stream_dir_name(stream));
            streams.insert(stream, StreamState::open(dir)?);
        }
        let sink = Self {
            options,
            redactor,
            clock,
            streams: Mutex::new(streams),
        };
        sink.enforce_retention()?;
        Ok(sink)
    }

    /// Stosuje retencję we wszystkich strumieniach; zwraca liczbę usuniętych segmentów.
    pub fn enforce_retention(&self) -> Result<usize, LogError> {
        let now = self.clock.now();
        let mut streams = self.lock();
        let mut removed = 0;
        for (stream, state) in streams.iter_mut() {
            if let Some(days) = self.options.limits(*stream).retention_days {
                removed += state.enforce_retention(now - Duration::days(i64::from(days)))?;
            }
        }
        Ok(removed)
    }

    /// Bajty zajęte przez strumień.
    pub fn disk_usage(&self, stream: LogStream) -> u64 {
        self.lock().get(&stream).map_or(0, StreamState::total_bytes)
    }

    /// Liczba segmentów strumienia.
    pub fn segment_count(&self, stream: LogStream) -> usize {
        self.lock().get(&stream).map_or(0, |s| s.segments.len())
    }

    fn lock(&self) -> MutexGuard<'_, BTreeMap<LogStream, StreamState>> {
        self.streams.lock().unwrap_or_else(|p| p.into_inner())
    }

    fn write_record(&self, stream: LogStream, event: &Event) -> Result<RecordRef, LogError> {
        let mut clean = event.clone();
        self.redactor.redact_value(&mut clean.payload);
        let now = self.clock.now();
        let limits = self.options.limits(stream);
        let mut streams = self.lock();
        let state = streams
            .get_mut(&stream)
            .ok_or_else(|| LogError::Io(format!("nieznany strumień {stream:?}")))?;
        let seq = state.next_seq;
        let record = DiskRecord {
            seq,
            schema_version: EVENT_SCHEMA_VERSION,
            written_at: now,
            event: clean,
        };
        let mut line = serde_json::to_vec(&record).map_err(|e| io_err("serializacja", e))?;
        line.push(b'\n');
        let len = line.len() as u64;
        if len > limits.disk_limit_bytes {
            return Err(LogError::DiskLimit(stream));
        }
        let max_segment = self.options.max_segment_bytes;
        if state.needs_rotation(len, max_segment) {
            if let Some(days) = limits.retention_days {
                state.enforce_retention(now - Duration::days(i64::from(days)))?;
            }
            if state.needs_rotation(len, max_segment) {
                state.rotate()?;
            }
        }
        let file = state.active_file()?;
        file.write_all(&line)
            .and_then(|()| file.flush())
            .map_err(|e| io_err("zapis rekordu", e))?;
        if let Some(active) = state.segments.last_mut() {
            active.bytes += len;
            active.last_written = Some(now);
        }
        state.next_seq += 1;
        while state.total_bytes() > limits.disk_limit_bytes && state.segments.len() > 1 {
            state.remove_oldest()?;
        }
        Ok(RecordRef { stream, seq })
    }

    fn read_matching(&self, query: &LogQuery) -> Result<Vec<LogRecord>, LogError> {
        let streams = self.lock();
        let mut out = Vec::new();
        for (stream, state) in streams.iter() {
            if query.stream.is_some_and(|s| s != *stream) {
                continue;
            }
            for (i, segment) in state.segments.iter().enumerate() {
                let next_first = state.segments.get(i + 1).map(|s| s.first_seq);
                let skip =
                    matches!((query.from_seq, next_first), (Some(from), Some(n)) if n <= from);
                if skip {
                    continue;
                }
                for rec in read_records(&segment.path)? {
                    let reference = RecordRef {
                        stream: *stream,
                        seq: rec.seq,
                    };
                    if query.matches(&reference, &rec.event) {
                        out.push(LogRecord {
                            reference,
                            event: rec.event,
                            schema_version: rec.schema_version,
                        });
                    }
                }
            }
        }
        Ok(out)
    }
}

#[async_trait]
impl LogSink for FileLogSink {
    async fn append(&self, stream: LogStream, event: &Event) -> Result<RecordRef, LogError> {
        self.write_record(stream, event)
    }

    async fn query(&self, query: LogQuery) -> Result<Vec<LogRecord>, LogError> {
        let mut records = self.read_matching(&query)?;
        records.sort_by(|a, b| {
            (a.event.ts, a.reference.stream, a.reference.seq).cmp(&(
                b.event.ts,
                b.reference.stream,
                b.reference.seq,
            ))
        });
        if let Some(limit) = query.limit {
            records.truncate(limit);
        }
        Ok(records)
    }
}
