//! [`DiskDownloads`] — kwarantanna pobrań na dysku (`DownloadStore`, `tools-net`): katalog
//! tworzony i sprawdzany (ścieżka rzeczywista = podana — bez dowiązań i junction na całej
//! ścieżce), plik częściowy `*.part` tworzony jako nowy, znacznik MOTW (`Zone.Identifier`,
//! Windows) zapisany na pliku częściowym (strumień przechodzi z plikiem), nazwa końcowa przez
//! dowiązanie twarde bez nadpisania (kolizja → ` (n)`), porzucenie usuwa plik częściowy.

use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use platform_apps_contract::{
    DownloadError, DownloadSink, DownloadStore, numbered_name, sanitize_file_name, zone_identifier,
};

/// Najwięcej prób nazwy przy kolizjach.
const MAX_NUMBERED: u32 = 1_000;

static COUNTER: AtomicU64 = AtomicU64::new(0);

/// Kwarantanna pobrań na dysku.
#[derive(Debug, Clone, Copy, Default)]
pub struct DiskDownloads;

fn io(e: &std::io::Error) -> DownloadError {
    DownloadError::Io(e.to_string())
}

/// Postać porównawcza ścieżki: bez `\\?\`, na Windows bez rozróżniania wielkości liter.
fn comparable(p: &Path) -> String {
    let s = p.to_string_lossy();
    let s = s.strip_prefix(r"\\?\").unwrap_or(&s);
    let s = s.trim_end_matches(['\\', '/']);
    if cfg!(windows) {
        s.to_lowercase()
    } else {
        s.to_owned()
    }
}

fn unsafe_dir(dir: &Path) -> DownloadError {
    DownloadError::Unsafe(format!(
        "{} prowadzi przez dowiązanie albo junction",
        dir.display()
    ))
}

/// Katalog istnieje (brakujące komponenty tworzone po kolei, dopiero gdy najgłębszy istniejący
/// przodek ma ścieżkę rzeczywistą równą podanej) i jego ścieżka rzeczywista jest równa podanej.
fn checked_dir(dir: &Path) -> Result<(), DownloadError> {
    if !dir.is_absolute() {
        return Err(DownloadError::Unsafe(format!(
            "{} nie jest ścieżką bezwzględną",
            dir.display()
        )));
    }
    let mut existing = dir;
    let mut missing = Vec::new();
    loop {
        match std::fs::symlink_metadata(existing) {
            Ok(_) => break,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                missing.push(existing.file_name().ok_or_else(|| unsafe_dir(dir))?);
                existing = existing.parent().ok_or_else(|| unsafe_dir(dir))?;
            }
            Err(e) => return Err(io(&e)),
        }
    }
    let real = std::fs::canonicalize(existing).map_err(|e| io(&e))?;
    if comparable(&real) != comparable(existing) {
        return Err(unsafe_dir(dir));
    }
    let mut cur = existing.to_path_buf();
    for name in missing.iter().rev() {
        cur.push(name);
        match std::fs::create_dir(&cur) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(e) => return Err(io(&e)),
        }
    }
    let real = std::fs::canonicalize(dir).map_err(|e| io(&e))?;
    if comparable(&real) != comparable(dir) || !real.is_dir() {
        return Err(unsafe_dir(dir));
    }
    Ok(())
}

fn zone_stream(path: &Path) -> PathBuf {
    let mut s = path.as_os_str().to_os_string();
    s.push(":Zone.Identifier");
    PathBuf::from(s)
}

impl DownloadStore for DiskDownloads {
    fn begin(&self, dir: &Path, name: &str) -> Result<Box<dyn DownloadSink>, DownloadError> {
        if name.is_empty() || name != sanitize_file_name(name) {
            return Err(DownloadError::Unsafe(format!("nazwa „{name}”")));
        }
        checked_dir(dir)?;
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos());
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let part = dir.join(format!("{name}.{nanos:x}-{n}.part"));
        let file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&part)
            .map_err(|e| io(&e))?;
        Ok(Box::new(DiskSink {
            file: Some(file),
            part,
            dir: dir.to_path_buf(),
            name: name.to_owned(),
            done: false,
        }))
    }
}

struct DiskSink {
    file: Option<File>,
    part: PathBuf,
    dir: PathBuf,
    name: String,
    done: bool,
}

impl DiskSink {
    /// Nazwa końcowa bez nadpisania: dowiązanie twarde (atomowe „utwórz, jeśli nie istnieje”),
    /// a gdy system plików ich nie ma — przemianowanie po sprawdzeniu.
    fn place(&self) -> Result<PathBuf, DownloadError> {
        for n in 1..=MAX_NUMBERED {
            let candidate = if n == 1 {
                self.dir.join(&self.name)
            } else {
                self.dir.join(numbered_name(&self.name, n))
            };
            match std::fs::hard_link(&self.part, &candidate) {
                Ok(()) => {
                    let _ = std::fs::remove_file(&self.part);
                    return Ok(candidate);
                }
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(_) if !candidate.exists() => {
                    std::fs::rename(&self.part, &candidate).map_err(|e| io(&e))?;
                    return Ok(candidate);
                }
                Err(_) => {}
            }
        }
        Err(DownloadError::Io("za dużo plików o tej nazwie".into()))
    }
}

impl DownloadSink for DiskSink {
    fn write(&mut self, chunk: &[u8]) -> Result<(), DownloadError> {
        match self.file.as_mut() {
            Some(f) => f.write_all(chunk).map_err(|e| io(&e)),
            None => Err(DownloadError::Io("plik zamknięty".into())),
        }
    }

    fn commit(mut self: Box<Self>, source_url: &str) -> Result<PathBuf, DownloadError> {
        if let Some(f) = self.file.take() {
            f.sync_all().map_err(|e| io(&e))?;
        }
        if cfg!(windows) {
            std::fs::write(zone_stream(&self.part), zone_identifier(source_url))
                .map_err(|e| io(&e))?;
        }
        let path = self.place()?;
        self.done = true;
        Ok(path)
    }
}

impl Drop for DiskSink {
    fn drop(&mut self) {
        self.file.take();
        if !self.done {
            let _ = std::fs::remove_file(&self.part);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn root() -> (tempfile::TempDir, PathBuf) {
        let t = tempfile::tempdir().unwrap();
        let real = std::fs::canonicalize(t.path()).unwrap();
        (t, real)
    }

    #[test]
    fn writes_new_files_without_overwriting() {
        let (_t, root) = root();
        let dir = root.join("Kwarantanna");
        let store = DiskDownloads;
        let mut paths = Vec::new();
        for body in [b"a".as_slice(), b"bb", b"ccc"] {
            let mut s = store.begin(&dir, "raport.pdf").unwrap();
            s.write(body).unwrap();
            paths.push(s.commit("https://x.pl/raport.pdf").unwrap());
        }
        assert_eq!(paths[0], dir.join("raport.pdf"));
        assert_eq!(paths[1], dir.join("raport (2).pdf"));
        assert_eq!(paths[2], dir.join("raport (3).pdf"));
        assert_eq!(std::fs::read(&paths[0]).unwrap(), b"a");
        assert_eq!(std::fs::read(&paths[2]).unwrap(), b"ccc");
        let left: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .filter_map(Result::ok)
            .filter(|e| e.path().extension().is_some_and(|x| x == "part"))
            .collect();
        assert!(left.is_empty(), "bez plików częściowych");
    }

    #[test]
    fn abandoned_download_leaves_nothing() {
        let (_t, root) = root();
        let dir = root.join("q");
        {
            let mut s = DiskDownloads.begin(&dir, "duzy.iso").unwrap();
            s.write(&[0u8; 1024]).unwrap();
        }
        assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 0);
    }

    #[test]
    fn rejects_unsafe_names_and_relative_dirs() {
        let (_t, root) = root();
        for bad in ["../x", "a:b", "", "x.txt."] {
            assert!(
                matches!(
                    DiskDownloads.begin(&root, bad),
                    Err(DownloadError::Unsafe(_))
                ),
                "{bad}"
            );
        }
        assert!(matches!(
            DiskDownloads.begin(Path::new("wzgledny"), "a.txt"),
            Err(DownloadError::Unsafe(_))
        ));
    }

    #[cfg(unix)]
    #[test]
    fn symlinked_quarantine_is_refused() {
        let (_t, root) = root();
        let target = root.join("Autostart");
        std::fs::create_dir_all(&target).unwrap();
        let link = root.join("Kwarantanna");
        std::os::unix::fs::symlink(&target, &link).unwrap();
        assert!(matches!(
            DiskDownloads.begin(&link, "evil.bat"),
            Err(DownloadError::Unsafe(_))
        ));
        assert!(matches!(
            DiskDownloads.begin(&link.join("pod"), "evil.bat"),
            Err(DownloadError::Unsafe(_))
        ));
        assert_eq!(
            std::fs::read_dir(&target).unwrap().count(),
            0,
            "nic nie powstało po drugiej stronie dowiązania"
        );
    }
}
