//! [`HttpFeed`] — manifest kanału i paczki przez HTTPS (`reqwest` + rustls, główne certyfikaty
//! systemu). Pobieranie **wznawiane** z pliku częściowego (`Range: bytes=<n>-`; `206` = dopisz,
//! `200` = serwer bez zakresów → od początku, `416` = plik już kompletny). Przekierowania tylko
//! na `https://` (≤ 5). Bez telemetrii: stały `User-Agent` bez wersji i identyfikatorów, bez
//! ciasteczek. Manifest ≤ 1 MiB; adresy względne paczek rozwiązywane wobec katalogu manifestów.

use std::path::Path;
use std::time::Duration;

use async_trait::async_trait;
use futures_util::StreamExt;
use reqwest::StatusCode;
use reqwest::header::{CONTENT_LENGTH, CONTENT_RANGE, RANGE};
use tokio::io::AsyncWriteExt;
use updater_contract::{
    Channel, DownloadProgress, Release, ReleaseFeed, ReleaseManifest, UpdaterError, is_allowed_url,
    manifest_url, resolve_url, validate_manifest,
};

/// Limit rozmiaru manifestu.
pub const MAX_MANIFEST_BYTES: usize = 1 << 20;
/// Limit paczki (twardy, niezależnie od nagłówków serwera).
pub const MAX_PACKAGE_BYTES: u64 = 2 << 30;
const USER_AGENT: &str = "Alfa-Updater";

/// Źródło wydań przez HTTPS.
pub struct HttpFeed {
    client: reqwest::Client,
    feed_url: String,
    allow_loopback_http: bool,
}

fn net(e: impl std::fmt::Display) -> UpdaterError {
    UpdaterError::network(e)
}

impl HttpFeed {
    /// Źródło dla adresu wydań (`https://…`; `allow_loopback_http` — tylko testy).
    pub fn new(feed_url: &str, allow_loopback_http: bool) -> Result<Self, UpdaterError> {
        manifest_url(feed_url, Channel::Stable, allow_loopback_http)?;
        let policy = reqwest::redirect::Policy::custom(move |attempt| {
            if attempt.previous().len() >= 5 {
                attempt.error("za dużo przekierowań")
            } else if is_allowed_url(attempt.url().as_str(), allow_loopback_http) {
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
            .https_only(!allow_loopback_http)
            .build()
            .map_err(net)?;
        Ok(Self {
            client,
            feed_url: feed_url.to_owned(),
            allow_loopback_http,
        })
    }

    async fn fetch_manifest(&self, url: &str) -> Result<ReleaseManifest, UpdaterError> {
        let resp = self.client.get(url).send().await.map_err(net)?;
        if !resp.status().is_success() {
            return Err(net(format!("manifest: HTTP {}", resp.status().as_u16())));
        }
        let mut body = Vec::new();
        let mut stream = resp.bytes_stream();
        while let Some(chunk) = stream.next().await {
            body.extend_from_slice(&chunk.map_err(net)?);
            if body.len() > MAX_MANIFEST_BYTES {
                return Err(UpdaterError::invalid("manifest większy niż 1 MiB"));
            }
        }
        serde_json::from_slice(&body)
            .map_err(|e| UpdaterError::invalid(format!("manifest wydań: {e}")))
    }
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

#[async_trait]
impl ReleaseFeed for HttpFeed {
    async fn manifest(&self, channel: Channel) -> Result<ReleaseManifest, UpdaterError> {
        let url = manifest_url(&self.feed_url, channel, self.allow_loopback_http)?;
        let manifest = self.fetch_manifest(&url).await?;
        validate_manifest(&manifest, channel)?;
        Ok(manifest)
    }

    async fn download(
        &self,
        release: &Release,
        dest: &Path,
        progress: &(dyn Fn(DownloadProgress) + Send + Sync),
    ) -> Result<(), UpdaterError> {
        // Adres względny — wobec katalogu manifestów (`<feed>/`, wspólny dla kanałów).
        let base = manifest_url(&self.feed_url, Channel::Stable, self.allow_loopback_http)?;
        let url = resolve_url(&base, &release.url, self.allow_loopback_http)?;
        let offset = tokio::fs::metadata(dest).await.map_or(0, |m| m.len());
        let mut req = self.client.get(&url);
        if offset > 0 {
            req = req.header(RANGE, format!("bytes={offset}-"));
        }
        let resp = req.send().await.map_err(net)?;
        let (append, start) = match resp.status() {
            StatusCode::PARTIAL_CONTENT if offset > 0 => (true, offset),
            StatusCode::RANGE_NOT_SATISFIABLE if offset > 0 => return Ok(()),
            s if s.is_success() => (false, 0),
            s => return Err(net(format!("paczka: HTTP {}", s.as_u16()))),
        };
        let total = total_from(&resp, start);
        if total.is_some_and(|t| t > MAX_PACKAGE_BYTES) {
            return Err(UpdaterError::unsafe_package("paczka większa niż 2 GiB"));
        }
        let mut file = tokio::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .append(append)
            .truncate(!append)
            .open(dest)
            .await?;
        let mut bytes = start;
        progress(DownloadProgress {
            downloaded: bytes,
            total,
            resumed: append,
        });
        let mut body = resp.bytes_stream();
        while let Some(chunk) = body.next().await {
            let chunk = match chunk {
                Ok(c) => c,
                Err(e) => {
                    file.flush().await?;
                    return Err(net(format!("pobieranie przerwane po {bytes} B: {e}")));
                }
            };
            file.write_all(&chunk).await?;
            // `tokio::fs::File` pisze w tle: bez `flush` postęp (i offset wznowienia po
            // anulowaniu) mógłby wyprzedzić dane faktycznie zapisane w pliku częściowym.
            file.flush().await?;
            bytes = bytes.saturating_add(chunk.len() as u64);
            if bytes > total.unwrap_or(MAX_PACKAGE_BYTES) {
                drop(file);
                let _ = tokio::fs::remove_file(dest).await;
                return Err(UpdaterError::unsafe_package(
                    "paczka większa niż zadeklarowana",
                ));
            }
            progress(DownloadProgress {
                downloaded: bytes,
                total,
                resumed: append,
            });
        }
        file.flush().await?;
        file.sync_all().await?;
        match total {
            Some(t) if bytes < t => Err(net(format!("pobrano {bytes} z {t} B"))),
            _ => Ok(()),
        }
    }
}
