//! Menedżer pobierania modeli: wznawianie (HTTP Range) z pliku `.part`, SHA-256 liczony w locie,
//! weryfikacja z manifestem (albo zapis hasha przy pierwszym pobraniu), atomowa zmiana nazwy.
//! Katalog modeli wstrzykiwany (`%LOCALAPPDATA%\Alfa\models` w produkcji).

use std::path::{Path, PathBuf};
use std::time::Duration;

use futures_util::StreamExt;
use providers_contract::CancellationToken;
use reqwest::StatusCode;
use reqwest::header::{CONTENT_LENGTH, CONTENT_RANGE, RANGE};
use sha2::{Digest, Sha256};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use crate::error::{DownloadProgress, LocalError};
use crate::manifest::ModelEntry;

/// Maksymalna liczba automatycznych wznowień w jednym wywołaniu.
pub const MAX_RESUMES: u32 = 3;

/// Wynik pobierania.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Downloaded {
    /// Ścieżka pliku modelu.
    pub path: PathBuf,
    /// SHA-256 (hex, małe litery).
    pub sha256: String,
    /// Czy hash zweryfikowano z manifestem (inaczej zapisany przy pierwszym pobraniu).
    pub verified: bool,
}

/// Menedżer pobierania.
pub struct Downloader {
    client: reqwest::Client,
    dir: PathBuf,
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Plik z zapisanym hashem obok modelu.
pub fn hash_path(model: &Path) -> PathBuf {
    let mut p = model.as_os_str().to_owned();
    p.push(".sha256");
    PathBuf::from(p)
}

/// Plik częściowy.
pub fn part_path(model: &Path) -> PathBuf {
    let mut p = model.as_os_str().to_owned();
    p.push(".part");
    PathBuf::from(p)
}

/// Czy model jest zainstalowany: plik + zapisany hash (zgodny z manifestem, jeśli ten go ma).
pub fn installed(dir: &Path, entry: &ModelEntry) -> bool {
    let path = dir.join(&entry.file);
    let Ok(recorded) = std::fs::read_to_string(hash_path(&path)) else {
        return false;
    };
    let recorded = recorded.trim();
    path.is_file()
        && recorded.len() == 64
        && (entry.sha256.is_empty() || recorded.eq_ignore_ascii_case(&entry.sha256))
}

async fn hash_existing(path: &Path, hasher: &mut Sha256) -> Result<u64, LocalError> {
    let mut file = tokio::fs::File::open(path).await?;
    let mut buf = vec![0u8; 1 << 16];
    let mut total = 0u64;
    loop {
        let n = file.read(&mut buf).await?;
        if n == 0 {
            return Ok(total);
        }
        hasher.update(&buf[..n]);
        total += n as u64;
    }
}

fn total_from(resp: &reqwest::Response, offset: u64) -> Option<u64> {
    let header = |name| resp.headers().get(name).and_then(|v| v.to_str().ok());
    if let Some(range) = header(CONTENT_RANGE) {
        return range.rsplit('/').next().and_then(|t| t.parse().ok());
    }
    header(CONTENT_LENGTH)
        .and_then(|l| l.parse::<u64>().ok())
        .map(|l| l + offset)
}

enum Step {
    Complete,
    Interrupted(String),
}

impl Downloader {
    /// Menedżer dla katalogu modeli.
    pub fn new(dir: impl Into<PathBuf>) -> Result<Self, LocalError> {
        let client = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(15))
            .build()
            .map_err(|e| LocalError::Download(e.to_string()))?;
        Ok(Self {
            client,
            dir: dir.into(),
        })
    }

    /// Katalog modeli.
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// Pobiera model (wznawia `.part`, do `MAX_RESUMES` automatycznych wznowień po przerwaniu).
    pub async fn download(
        &self,
        entry: &ModelEntry,
        cancel: &CancellationToken,
        progress: &(dyn Fn(DownloadProgress) + Send + Sync),
    ) -> Result<Downloaded, LocalError> {
        entry.validate()?;
        tokio::fs::create_dir_all(&self.dir).await?;
        let path = self.dir.join(&entry.file);
        if installed(&self.dir, entry) {
            let sha256 = tokio::fs::read_to_string(hash_path(&path)).await?;
            return Ok(Downloaded {
                path,
                sha256: sha256.trim().to_owned(),
                verified: !entry.sha256.is_empty(),
            });
        }
        let part = part_path(&path);
        let mut resumes = 0u32;
        loop {
            match self.fetch(entry, &part, cancel, progress).await? {
                Step::Complete => break,
                Step::Interrupted(why) if resumes < MAX_RESUMES => {
                    resumes += 1;
                    tracing::warn!(model = %entry.id, resumes, %why, "pobieranie przerwane — wznawiam");
                }
                Step::Interrupted(why) => return Err(LocalError::Download(why)),
            }
        }
        self.finish(entry, &path, &part).await
    }

    async fn finish(
        &self,
        entry: &ModelEntry,
        path: &Path,
        part: &Path,
    ) -> Result<Downloaded, LocalError> {
        let mut hasher = Sha256::new();
        hash_existing(part, &mut hasher).await?;
        let actual = hex(&hasher.finalize());
        let verified = !entry.sha256.is_empty();
        if verified && !actual.eq_ignore_ascii_case(&entry.sha256) {
            // Uszkodzony albo podmieniony plik — nie zostawiamy go do wznowienia.
            let _ = tokio::fs::remove_file(part).await;
            return Err(LocalError::HashMismatch {
                file: entry.file.clone(),
                expected: entry.sha256.to_ascii_lowercase(),
                actual,
            });
        }
        if !verified {
            tracing::warn!(model = %entry.id, sha256 = %actual,
                "manifest bez SHA-256: hash zapisany przy pierwszym pobraniu (zaufanie przy pierwszym użyciu)");
        }
        tokio::fs::rename(part, path).await?;
        tokio::fs::write(hash_path(path), format!("{actual}\n")).await?;
        Ok(Downloaded {
            path: path.to_owned(),
            sha256: actual,
            verified,
        })
    }

    async fn fetch(
        &self,
        entry: &ModelEntry,
        part: &Path,
        cancel: &CancellationToken,
        progress: &(dyn Fn(DownloadProgress) + Send + Sync),
    ) -> Result<Step, LocalError> {
        let offset = tokio::fs::metadata(part).await.map_or(0, |m| m.len());
        let mut req = self.client.get(&entry.url);
        if offset > 0 {
            req = req.header(RANGE, format!("bytes={offset}-"));
        }
        let resp = tokio::select! {
            () = cancel.cancelled() => return Err(LocalError::Cancelled),
            r = req.send() => match r {
                Ok(r) => r,
                Err(e) => return Ok(Step::Interrupted(e.to_string())),
            },
        };
        let status = resp.status();
        let (append, start) = match status {
            StatusCode::PARTIAL_CONTENT if offset > 0 => (true, offset),
            StatusCode::RANGE_NOT_SATISFIABLE if offset > 0 => return Ok(Step::Complete),
            s if s.is_success() => (false, 0),
            s => return Err(LocalError::Download(format!("HTTP {}", s.as_u16()))),
        };
        let total = total_from(&resp, start);
        let mut file = tokio::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .append(append)
            .truncate(!append)
            .open(part)
            .await?;
        let mut bytes = start;
        progress(DownloadProgress {
            bytes,
            total,
            resumed: append,
        });
        let mut body = resp.bytes_stream();
        loop {
            let next = tokio::select! {
                () = cancel.cancelled() => {
                    file.flush().await?;
                    return Err(LocalError::Cancelled);
                }
                n = body.next() => n,
            };
            match next {
                Some(Ok(chunk)) => {
                    file.write_all(&chunk).await?;
                    bytes += chunk.len() as u64;
                    progress(DownloadProgress {
                        bytes,
                        total,
                        resumed: append,
                    });
                }
                Some(Err(e)) => {
                    file.flush().await?;
                    return Ok(Step::Interrupted(e.to_string()));
                }
                None => break,
            }
        }
        file.flush().await?;
        match total {
            Some(t) if bytes < t => Ok(Step::Interrupted(format!("pobrano {bytes} z {t} B"))),
            Some(t) if bytes > t => {
                let _ = tokio::fs::remove_file(part).await;
                Err(LocalError::Download(format!(
                    "plik większy niż deklarowany ({bytes} > {t} B)"
                )))
            }
            _ => Ok(Step::Complete),
        }
    }
}
