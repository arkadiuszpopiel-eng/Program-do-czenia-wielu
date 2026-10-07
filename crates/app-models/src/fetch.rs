//! Pobieranie pliku przez HTTPS z wznawianiem: plik częściowy `<plik>.part`, `Range: bytes=<n>-`
//! (`206` = dopisz, `200` = serwer bez zakresów → od początku, `416` = plik już kompletny),
//! SHA-256 liczony w locie (z przeliczeniem istniejącej części), twardy limit rozmiaru, do
//! [`MAX_RESUMES`] automatycznych wznowień po zerwaniu, anulowanie. Przekierowania tylko na
//! `https://` (≤ 5). Bez telemetrii: stały `User-Agent` bez wersji, bez ciasteczek.

use std::path::{Path, PathBuf};
use std::time::Duration;

use futures_util::StreamExt;
use reqwest::StatusCode;
use reqwest::header::{CONTENT_LENGTH, CONTENT_RANGE, RANGE};
use sha2::{Digest, Sha256};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio_util::sync::CancellationToken;

/// Automatyczne wznowienia jednego pliku po przerwanym strumieniu.
pub const MAX_RESUMES: u32 = 3;
const USER_AGENT: &str = "Alfa-Models";

/// Błąd pobierania.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FetchError {
    /// Anulowano (plik częściowy zostaje do wznowienia).
    Cancelled,
    /// Sieć / serwer (po wyczerpaniu wznowień).
    Network(String),
    /// Odpowiedź niezgodna z zasadami (adres, limit rozmiaru) — bez ponawiania.
    Rejected(String),
    /// Dysk.
    Io(String),
}

impl std::fmt::Display for FetchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Cancelled => write!(f, "anulowano"),
            Self::Network(why) => write!(f, "sieć: {why}"),
            Self::Rejected(why) => write!(f, "odrzucono: {why}"),
            Self::Io(why) => write!(f, "dysk: {why}"),
        }
    }
}

fn io(path: &Path, e: impl std::fmt::Display) -> FetchError {
    FetchError::Io(format!("{}: {e}", path.display()))
}

/// Plik częściowy.
pub fn part_path(path: &Path) -> PathBuf {
    let mut p = path.as_os_str().to_owned();
    p.push(".part");
    PathBuf::from(p)
}

/// SHA-256 (hex).
pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Czy adres jest dozwolony (`https://`; `http://127.0.0.1` tylko w testach).
pub fn allowed_url(url: &str, loopback_http: bool) -> bool {
    url.starts_with("https://")
        || (loopback_http
            && (url.starts_with("http://127.0.0.1:") || url.starts_with("http://localhost:")))
}

/// Klient HTTP menedżera.
pub struct Fetcher {
    client: reqwest::Client,
    loopback_http: bool,
}

enum Step {
    Done(String, u64),
    Interrupted(String),
}

fn total_from(resp: &reqwest::Response, offset: u64) -> Option<u64> {
    let header = |name| resp.headers().get(name).and_then(|v| v.to_str().ok());
    if let Some(range) = header(CONTENT_RANGE) {
        return range.rsplit('/').next().and_then(|t| t.parse().ok());
    }
    header(CONTENT_LENGTH)
        .and_then(|l| l.parse::<u64>().ok())
        .map(|l| l.saturating_add(offset))
}

impl Fetcher {
    /// Klient (`loopback_http` — tylko testy z serwerem na 127.0.0.1).
    pub fn new(loopback_http: bool) -> Result<Self, FetchError> {
        let policy = reqwest::redirect::Policy::custom(move |attempt| {
            if attempt.previous().len() >= 5 {
                attempt.error("za dużo przekierowań")
            } else if allowed_url(attempt.url().as_str(), loopback_http) {
                attempt.follow()
            } else {
                attempt.error("przekierowanie poza https://")
            }
        });
        let client = reqwest::Client::builder()
            .user_agent(USER_AGENT)
            .redirect(policy)
            .connect_timeout(Duration::from_secs(15))
            .read_timeout(Duration::from_secs(30))
            .https_only(!loopback_http)
            .build()
            .map_err(|e| FetchError::Network(e.to_string()))?;
        Ok(Self {
            client,
            loopback_http,
        })
    }

    /// Pobiera `url` do `path` (przez `<path>.part`); zwraca SHA-256 i rozmiar. Przerwanie zostawia
    /// plik częściowy; przekroczenie `limit` usuwa go.
    pub async fn fetch(
        &self,
        url: &str,
        path: &Path,
        limit: u64,
        cancel: &CancellationToken,
        progress: &(dyn Fn(u64, Option<u64>) + Send + Sync),
    ) -> Result<(String, u64), FetchError> {
        if !allowed_url(url, self.loopback_http) {
            return Err(FetchError::Rejected(format!("adres musi być https: {url}")));
        }
        let part = part_path(path);
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|e| io(parent, e))?;
        }
        let mut resumes = 0;
        loop {
            match self.attempt(url, &part, limit, cancel, progress).await? {
                Step::Done(sha, size) => {
                    tokio::fs::rename(&part, path)
                        .await
                        .map_err(|e| io(path, e))?;
                    return Ok((sha, size));
                }
                Step::Interrupted(why) if resumes < MAX_RESUMES => {
                    resumes += 1;
                    tracing::warn!(%why, resumes, "pobieranie przerwane — wznawiam");
                }
                Step::Interrupted(why) => return Err(FetchError::Network(why)),
            }
        }
    }

    async fn attempt(
        &self,
        url: &str,
        part: &Path,
        limit: u64,
        cancel: &CancellationToken,
        progress: &(dyn Fn(u64, Option<u64>) + Send + Sync),
    ) -> Result<Step, FetchError> {
        let mut hasher = Sha256::new();
        let mut offset = hash_existing(part, &mut hasher).await?;
        let mut req = self.client.get(url);
        if offset > 0 {
            req = req.header(RANGE, format!("bytes={offset}-"));
        }
        let resp = tokio::select! {
            () = cancel.cancelled() => return Err(FetchError::Cancelled),
            r = req.send() => match r {
                Ok(r) => r,
                Err(e) => return Ok(Step::Interrupted(e.to_string())),
            },
        };
        let append = match resp.status() {
            StatusCode::PARTIAL_CONTENT if offset > 0 => true,
            StatusCode::RANGE_NOT_SATISFIABLE if offset > 0 => {
                return Ok(Step::Done(hex(&hasher.finalize()), offset));
            }
            s if s.is_success() => false,
            s if s.is_server_error() || s == StatusCode::TOO_MANY_REQUESTS => {
                return Ok(Step::Interrupted(format!("HTTP {}", s.as_u16())));
            }
            s => return Err(FetchError::Rejected(format!("HTTP {}", s.as_u16()))),
        };
        if !append {
            hasher = Sha256::new();
            offset = 0;
        }
        let total = total_from(&resp, offset);
        if total.is_some_and(|t| t > limit) {
            let _ = tokio::fs::remove_file(part).await;
            return Err(FetchError::Rejected(format!(
                "plik większy niż limit {limit} B"
            )));
        }
        let mut file = tokio::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .append(append)
            .truncate(!append)
            .open(part)
            .await
            .map_err(|e| io(part, e))?;
        let mut done = offset;
        progress(done, total);
        let mut body = resp.bytes_stream();
        loop {
            let next = tokio::select! {
                () = cancel.cancelled() => {
                    file.flush().await.map_err(|e| io(part, e))?;
                    return Err(FetchError::Cancelled);
                }
                n = body.next() => n,
            };
            let chunk = match next {
                Some(Ok(chunk)) => chunk,
                Some(Err(e)) => {
                    file.flush().await.map_err(|e| io(part, e))?;
                    return Ok(Step::Interrupted(e.to_string()));
                }
                None => break,
            };
            done += chunk.len() as u64;
            if done > limit {
                drop(file);
                let _ = tokio::fs::remove_file(part).await;
                return Err(FetchError::Rejected(format!(
                    "plik większy niż limit {limit} B"
                )));
            }
            file.write_all(&chunk).await.map_err(|e| io(part, e))?;
            hasher.update(&chunk);
            progress(done, total);
        }
        file.flush().await.map_err(|e| io(part, e))?;
        match total {
            Some(t) if done < t => Ok(Step::Interrupted(format!("pobrano {done} z {t} B"))),
            Some(t) if done > t => {
                let _ = tokio::fs::remove_file(part).await;
                Err(FetchError::Rejected(format!(
                    "plik większy niż deklarowany ({done} > {t} B)"
                )))
            }
            _ => Ok(Step::Done(hex(&hasher.finalize()), done)),
        }
    }
}

/// Hashuje istniejący plik częściowy (do wznowienia); zwraca jego długość.
async fn hash_existing(part: &Path, hasher: &mut Sha256) -> Result<u64, FetchError> {
    let mut file = match tokio::fs::File::open(part).await {
        Ok(f) => f,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(0),
        Err(e) => return Err(io(part, e)),
    };
    let mut buf = vec![0u8; 1 << 16];
    let mut total = 0u64;
    loop {
        let n = file.read(&mut buf).await.map_err(|e| io(part, e))?;
        if n == 0 {
            return Ok(total);
        }
        hasher.update(&buf[..n]);
        total += n as u64;
    }
}

/// SHA-256 pliku (blokujące — w `spawn_blocking`).
pub fn sha256_file(path: &Path) -> Result<String, FetchError> {
    let mut file = std::fs::File::open(path).map_err(|e| io(path, e))?;
    let mut hasher = Sha256::new();
    std::io::copy(&mut file, &mut hasher).map_err(|e| io(path, e))?;
    Ok(hex(&hasher.finalize()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_https_or_test_loopback() {
        assert!(allowed_url("https://huggingface.co/x", false));
        assert!(!allowed_url("http://huggingface.co/x", false));
        assert!(!allowed_url("http://127.0.0.1:8080/x", false));
        assert!(allowed_url("http://127.0.0.1:8080/x", true));
        assert!(!allowed_url("http://10.0.0.1/x", true));
        assert!(!allowed_url("file:///etc/passwd", true));
        assert_eq!(part_path(Path::new("a/b.bin")), Path::new("a/b.bin.part"));
    }
}
