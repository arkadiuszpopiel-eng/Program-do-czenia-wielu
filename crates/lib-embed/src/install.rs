//! Instalator modelu z katalogu: pobieranie przez port [`Fetcher`] (adapter HTTP z `Range` w `app-*`,
//! jak menedżer `providers-local`), wznawianie z `.part`, SHA-256 liczony w locie, atomowa zmiana
//! nazwy, limit rozmiaru (2× rozmiar z katalogu), anulowanie; na końcu manifest `embed.json`.

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

use sha2::{Digest, Sha256};

use crate::catalog::{CatalogEntry, CatalogFile};
use crate::error::EmbedError;
use crate::manifest::{EmbedManifest, MANIFEST_FILE, sha256_file};

/// Automatyczne wznowienia jednego pliku po przerwanym strumieniu.
pub const MAX_RESUMES: u32 = 3;

/// Otwarty strumień pliku.
pub struct Fetched {
    /// Czy serwer wznowił od żądanego bajtu (HTTP 206); `false` → strumień od początku.
    pub resumed: bool,
    /// Całkowity rozmiar pliku, jeśli znany.
    pub total: Option<u64>,
    /// Treść.
    pub body: Box<dyn Read + Send>,
}

/// Port pobierania (adapter HTTP w `app-*`; atrapa w testach).
pub trait Fetcher: Send + Sync {
    /// Strumień pliku `url` od bajtu `offset` (`Range: bytes=offset-`).
    fn open(&self, url: &str, offset: u64) -> Result<Fetched, EmbedError>;
}

/// Polityka hashy plików katalogu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HashPolicy {
    /// Tylko pliki z przypiętym SHA-256 (produkcja).
    PinnedOnly,
    /// Plik bez przypiętego hasha — hash pierwszego pobrania trafia do manifestu (jawna zgoda w UI).
    TrustOnFirstUse,
}

/// Postęp instalacji.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstallProgress {
    /// Plik (ścieżka w katalogu modelu).
    pub file: String,
    /// Pobrane bajty pliku.
    pub done: u64,
    /// Rozmiar pliku, jeśli znany.
    pub total: Option<u64>,
}

/// Wynik instalacji.
#[derive(Debug, Clone, PartialEq)]
pub struct Installed {
    /// Ścieżka manifestu.
    pub manifest_path: PathBuf,
    /// Manifest.
    pub manifest: EmbedManifest,
    /// Czy wszystkie hashe były przypięte w katalogu.
    pub pinned: bool,
}

fn part_path(path: &Path) -> PathBuf {
    let mut p = path.as_os_str().to_owned();
    p.push(".part");
    PathBuf::from(p)
}

/// Wynik nieudanej próby pobierania.
enum Step {
    /// Przerwany strumień — wznowić od `.part`.
    Retry(String),
    /// Błąd bez ponawiania.
    Fatal(EmbedError),
}

fn fatal_io(path: &Path, e: impl std::fmt::Display) -> Step {
    Step::Fatal(EmbedError::io(path, e))
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Ścieżka manifestu, jeśli model jest zainstalowany (manifest poprawny, pliki obecne).
/// Pełna weryfikacja hashy następuje przy ładowaniu.
pub fn installed(dir: &Path) -> Option<PathBuf> {
    let path = dir.join(MANIFEST_FILE);
    let (m, base) = EmbedManifest::load(&path).ok()?;
    (base.join(&m.model.path).is_file() && base.join(&m.tokenizer.path).is_file()).then_some(path)
}

struct Download<'a> {
    fetcher: &'a dyn Fetcher,
    cancel: &'a AtomicBool,
    progress: &'a dyn Fn(InstallProgress),
}

impl Download<'_> {
    /// Pobiera jeden plik do `path`; zwraca jego SHA-256.
    fn file(&self, file: &CatalogFile, path: &Path) -> Result<String, EmbedError> {
        if path.is_file() {
            let actual = sha256_file(path)?;
            match file.sha256 {
                Some(pin) if !pin.eq_ignore_ascii_case(&actual) => {
                    std::fs::remove_file(path).map_err(|e| EmbedError::io(path, e))?;
                }
                _ => return Ok(actual),
            }
        }
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| EmbedError::io(parent, e))?;
        }
        let part = part_path(path);
        let limit = u64::from(file.size_mb.max(1)) * 2 * 1024 * 1024;
        let mut resumes = 0;
        let sha = loop {
            match self.attempt(file, &part, limit) {
                Ok(sha) => break sha,
                Err(Step::Retry(_)) if resumes < MAX_RESUMES => resumes += 1,
                Err(Step::Retry(why)) => return Err(EmbedError::Fetch(why)),
                Err(Step::Fatal(e)) => return Err(e),
            }
        };
        if let Some(pin) = file.sha256
            && !pin.eq_ignore_ascii_case(&sha)
        {
            let _ = std::fs::remove_file(&part);
            return Err(EmbedError::Hash {
                path: path.display().to_string(),
                expected: pin.to_ascii_lowercase(),
                actual: sha,
            });
        }
        std::fs::rename(&part, path).map_err(|e| EmbedError::io(path, e))?;
        Ok(sha)
    }

    /// Jedna próba: dokleja do `.part` od jego długości (albo od zera, gdy serwer nie wznawia).
    fn attempt(&self, file: &CatalogFile, part: &Path, limit: u64) -> Result<String, Step> {
        let mut hasher = Sha256::new();
        let mut offset = 0_u64;
        if part.is_file() {
            let mut existing = std::fs::File::open(part).map_err(|e| fatal_io(part, e))?;
            offset = std::io::copy(&mut existing, &mut hasher).map_err(|e| fatal_io(part, e))?;
        }
        let mut fetched = self.fetcher.open(file.url, offset).map_err(|e| match e {
            EmbedError::Fetch(why) => Step::Retry(why),
            other => Step::Fatal(other),
        })?;
        if offset > 0 && !fetched.resumed {
            hasher = Sha256::new();
            offset = 0;
        }
        let mut out = std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .append(offset > 0)
            .truncate(offset == 0)
            .open(part)
            .map_err(|e| fatal_io(part, e))?;
        let mut buf = vec![0_u8; 1 << 16];
        let mut done = offset;
        let mut reported = 0_u64;
        loop {
            if self.cancel.load(Ordering::SeqCst) {
                return Err(Step::Fatal(EmbedError::Cancelled));
            }
            let n = fetched
                .body
                .read(&mut buf)
                .map_err(|e| Step::Retry(e.to_string()))?;
            if n == 0 {
                break;
            }
            done += n as u64;
            if done > limit {
                drop(out);
                let _ = std::fs::remove_file(part);
                return Err(Step::Fatal(EmbedError::Fetch(format!(
                    "{}: plik większy niż limit {limit} B",
                    file.path
                ))));
            }
            out.write_all(&buf[..n]).map_err(|e| fatal_io(part, e))?;
            hasher.update(&buf[..n]);
            if done - reported >= 1 << 20 {
                reported = done;
                (self.progress)(InstallProgress {
                    file: file.path.into(),
                    done,
                    total: fetched.total,
                });
            }
        }
        out.flush().map_err(|e| fatal_io(part, e))?;
        if fetched.total.is_some_and(|t| t != done) {
            return Err(Step::Retry(format!("{}: niepełny plik", file.path)));
        }
        (self.progress)(InstallProgress {
            file: file.path.into(),
            done,
            total: fetched.total,
        });
        Ok(hex(&hasher.finalize()))
    }
}

/// Instaluje model `entry` w `dir` (pliki + `embed.json`). Przerwane pobieranie zostawia `.part`
/// (kolejne wywołanie wznawia); niezgodny hash usuwa plik częściowy.
pub fn install(
    entry: &CatalogEntry,
    dir: &Path,
    fetcher: &dyn Fetcher,
    policy: HashPolicy,
    cancel: &AtomicBool,
    progress: &dyn Fn(InstallProgress),
) -> Result<Installed, EmbedError> {
    let pinned = entry.files().iter().all(|f| f.sha256.is_some());
    if !pinned && policy == HashPolicy::PinnedOnly {
        return Err(EmbedError::Manifest(format!(
            "`{}`: brak przypiętego SHA-256 w katalogu — wymagana zgoda (TrustOnFirstUse)",
            entry.id
        )));
    }
    let download = Download {
        fetcher,
        cancel,
        progress,
    };
    let model_sha = download.file(&entry.model, &dir.join(entry.model.path))?;
    let tok_sha = download.file(&entry.tokenizer, &dir.join(entry.tokenizer.path))?;
    let manifest = entry.manifest(&model_sha, &tok_sha);
    manifest.validate()?;
    let path = dir.join(MANIFEST_FILE);
    let tmp = part_path(&path);
    std::fs::write(&tmp, manifest.to_json()).map_err(|e| EmbedError::io(&tmp, e))?;
    std::fs::rename(&tmp, &path).map_err(|e| EmbedError::io(&path, e))?;
    Ok(Installed {
        manifest_path: path,
        manifest,
        pinned,
    })
}
