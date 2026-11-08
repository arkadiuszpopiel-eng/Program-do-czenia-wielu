//! Plik dziennika z rotacją: `<katalog>/<proces>.<RRRR-MM-DD>.<NNN>.log` (data UTC).
//!
//! Nowy plik: zmiana dnia albo przekroczenie [`Rotation::max_file_bytes`]. Po każdej rotacji
//! (i przy otwarciu) usuwane są pliki tego procesu starsze niż [`Rotation::retention_days`]
//! oraz najstarsze ponad [`Rotation::max_files`] — dziennik zajmuje najwyżej
//! `max_files × max_file_bytes` na proces. Pliki innych procesów (inny prefiks) i inne pliki
//! katalogu (segmenty `core-log`) nie są dotykane.

use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use chrono::{DateTime, Duration, NaiveDate, Utc};

/// Źródło czasu (w testach — zegar wirtualny).
pub type Clock = Arc<dyn Fn() -> DateTime<Utc> + Send + Sync>;

/// Domyślny rozmiar pliku, po którym następuje rotacja: 10 MiB.
pub const DEFAULT_MAX_FILE_BYTES: u64 = 10 * 1024 * 1024;
/// Domyślna liczba plików procesu: 14 (≤ 140 MiB).
pub const DEFAULT_MAX_FILES: usize = 14;
/// Domyślna retencja (PLAN §13): 7 dni.
pub const DEFAULT_RETENTION_DAYS: u32 = 7;

/// Limity pliku dziennika.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rotation {
    /// Rozmiar pliku, po którym zaczyna się nowy.
    pub max_file_bytes: u64,
    /// Maksymalna liczba plików procesu (najstarsze usuwane; co najmniej 1).
    pub max_files: usize,
    /// Pliki z datą starszą niż tyle dni są usuwane.
    pub retention_days: u32,
}

impl Default for Rotation {
    fn default() -> Self {
        Self {
            max_file_bytes: DEFAULT_MAX_FILE_BYTES,
            max_files: DEFAULT_MAX_FILES,
            retention_days: DEFAULT_RETENTION_DAYS,
        }
    }
}

/// Plik dziennika procesu w katalogu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogFile {
    /// Dzień (UTC) z nazwy pliku.
    pub date: NaiveDate,
    /// Numer kolejny w danym dniu.
    pub seq: u32,
    /// Ścieżka.
    pub path: PathBuf,
}

/// Nazwa pliku: `<proces>.<RRRR-MM-DD>.<NNN>.log`.
pub fn file_name(process: &str, date: NaiveDate, seq: u32) -> String {
    format!("{process}.{}.{seq:03}.log", date.format("%Y-%m-%d"))
}

/// Dzień i numer z nazwy pliku procesu (`None` — plik innego procesu albo inny plik).
pub fn parse_name(process: &str, name: &str) -> Option<(NaiveDate, u32)> {
    let rest = name.strip_prefix(process)?.strip_prefix('.')?;
    let (date, seq) = rest.strip_suffix(".log")?.split_once('.')?;
    if date.len() != 10 || seq.len() < 3 || !seq.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let date = NaiveDate::parse_from_str(date, "%Y-%m-%d").ok()?;
    Some((date, seq.parse().ok()?))
}

/// Pliki procesu w katalogu, rosnąco po (dzień, numer).
pub fn list(dir: &Path, process: &str) -> io::Result<Vec<LogFile>> {
    let mut out = Vec::new();
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let name = entry.file_name();
        let Some((date, seq)) = name.to_str().and_then(|n| parse_name(process, n)) else {
            continue;
        };
        if entry.file_type()?.is_file() {
            out.push(LogFile {
                date,
                seq,
                path: entry.path(),
            });
        }
    }
    out.sort_by_key(|f| (f.date, f.seq));
    Ok(out)
}

struct Active {
    date: NaiveDate,
    path: PathBuf,
    file: File,
    bytes: u64,
}

/// Plik dziennika z rotacją (zapis synchroniczny — wywołujący trzyma zamek).
pub struct RollingFile {
    dir: PathBuf,
    process: String,
    rotation: Rotation,
    clock: Clock,
    active: Option<Active>,
}

impl RollingFile {
    /// Tworzy katalog i stosuje retencję; plik powstaje przy pierwszym zapisie.
    pub fn open(dir: PathBuf, process: &str, rotation: Rotation, clock: Clock) -> io::Result<Self> {
        fs::create_dir_all(&dir)?;
        let file = Self {
            dir,
            process: process.to_owned(),
            rotation,
            clock,
            active: None,
        };
        file.prune(file.today())?;
        Ok(file)
    }

    /// Katalog dziennika.
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// Bieżący plik (po pierwszym zapisie).
    pub fn current(&self) -> Option<&Path> {
        self.active.as_ref().map(|a| a.path.as_path())
    }

    fn today(&self) -> NaiveDate {
        (self.clock)().date_naive()
    }

    /// Zmienia retencję i od razu ją stosuje; zwraca liczbę usuniętych plików.
    pub fn set_retention_days(&mut self, days: u32) -> io::Result<usize> {
        self.rotation.retention_days = days;
        self.prune(self.today())
    }

    /// Dopisuje linię (z `\n`), w razie potrzeby zaczynając nowy plik.
    pub fn write_line(&mut self, line: &[u8]) -> io::Result<()> {
        let today = self.today();
        let len = line.len() as u64;
        let max = self.rotation.max_file_bytes;
        let rotate = match &self.active {
            None => true,
            Some(a) => a.date != today || (a.bytes > 0 && a.bytes.saturating_add(len) > max),
        };
        if rotate {
            self.rotate(today, len)?;
        }
        let active = self
            .active
            .as_mut()
            .ok_or_else(|| io::Error::other("brak pliku dziennika"))?;
        active.file.write_all(line)?;
        active.bytes = active.bytes.saturating_add(len);
        Ok(())
    }

    /// Następny plik dnia: ostatni plik dnia, jeśli się zmieści, inaczej kolejny numer.
    fn rotate(&mut self, today: NaiveDate, len: u64) -> io::Result<()> {
        self.active = None;
        let last = list(&self.dir, &self.process)?
            .into_iter()
            .rfind(|f| f.date == today);
        let seq = match last {
            None => 0,
            Some(f) => {
                let size = fs::metadata(&f.path).map_or(u64::MAX, |m| m.len());
                let fits = size == 0 || size.saturating_add(len) <= self.rotation.max_file_bytes;
                if fits { f.seq } else { f.seq.saturating_add(1) }
            }
        };
        let path = self.dir.join(file_name(&self.process, today, seq));
        let file = OpenOptions::new().create(true).append(true).open(&path)?;
        let bytes = file.metadata()?.len();
        self.active = Some(Active {
            date: today,
            path,
            file,
            bytes,
        });
        self.prune(today)?;
        Ok(())
    }

    /// Usuwa pliki starsze niż retencja i najstarsze ponad limit liczby (nigdy bieżącego).
    /// Zwraca liczbę usuniętych; plik zablokowany przez inny proces zostaje do następnej próby.
    pub fn prune(&self, today: NaiveDate) -> io::Result<usize> {
        let cutoff = today - Duration::days(i64::from(self.rotation.retention_days));
        let current = self.current().map(Path::to_path_buf);
        let mut kept = Vec::new();
        let mut removed = 0;
        for f in list(&self.dir, &self.process)? {
            let is_current = current.as_ref() == Some(&f.path);
            if f.date < cutoff && !is_current {
                removed += usize::from(fs::remove_file(&f.path).is_ok());
            } else {
                kept.push((f, is_current));
            }
        }
        let max = self.rotation.max_files.max(1);
        let mut excess = kept.len().saturating_sub(max);
        for (f, is_current) in kept {
            if excess == 0 {
                break;
            }
            if !is_current {
                removed += usize::from(fs::remove_file(&f.path).is_ok());
                excess -= 1;
            }
        }
        Ok(removed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_roundtrip_and_reject_other_files() {
        let d = NaiveDate::from_ymd_opt(2026, 10, 6).unwrap();
        assert_eq!(file_name("alfa", d, 7), "alfa.2026-10-06.007.log");
        assert_eq!(parse_name("alfa", "alfa.2026-10-06.007.log"), Some((d, 7)));
        assert_eq!(
            parse_name("alfa", "alfa.2026-10-06.1234.log"),
            Some((d, 1234))
        );
        for other in [
            "alfa-broker.2026-10-06.000.log",
            "alfa.2026-10-06.log",
            "alfa.2026-13-06.000.log",
            "alfa.2026-10-06.00x.log",
            "alfa.2026-10-06.000.ndjson",
            "00000000000000000000.ndjson",
        ] {
            assert_eq!(parse_name("alfa", other), None, "{other}");
        }
        assert_eq!(
            parse_name("alfa-broker", "alfa-broker.2026-10-06.000.log"),
            Some((d, 0))
        );
    }
}
