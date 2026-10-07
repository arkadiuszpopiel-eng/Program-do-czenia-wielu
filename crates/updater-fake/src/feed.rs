//! Atrapa źródła wydań ([`ReleaseFeed`]): manifesty kanałów i paczki w pamięci, wznawianie od
//! rozmiaru pliku docelowego (jak HTTP Range), jednorazowe przerwanie po N bajtach, wyłączenie
//! sieci, dziennik żądań. Deterministyczna — bez sieci i zegara.

use std::collections::BTreeMap;
use std::io::Write;
use std::path::Path;
use std::sync::{Mutex, MutexGuard, PoisonError};

use async_trait::async_trait;
use updater_contract::{
    Channel, DownloadProgress, RELEASES_SCHEMA, Release, ReleaseFeed, ReleaseManifest,
    UpdaterError, validate_manifest,
};

#[derive(Debug, Default)]
struct State {
    manifests: BTreeMap<&'static str, ReleaseManifest>,
    packages: BTreeMap<String, Vec<u8>>,
    cut_next_after: Option<u64>,
    offline: bool,
    requests: Vec<(String, u64)>,
}

/// Atrapa źródła wydań.
#[derive(Debug, Default)]
pub struct FakeFeed {
    state: Mutex<State>,
}

impl FakeFeed {
    /// Puste źródło.
    pub fn new() -> Self {
        Self::default()
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Publikuje wydania w kanale (paczki pod `release.url`).
    pub fn publish(&self, channel: Channel, releases: Vec<(Release, Vec<u8>)>) {
        let mut st = self.lock();
        for (r, bytes) in &releases {
            st.packages.insert(r.url.clone(), bytes.clone());
        }
        st.manifests.insert(
            channel.as_str(),
            ReleaseManifest {
                schema: RELEASES_SCHEMA,
                channel: channel.as_str().to_owned(),
                releases: releases.into_iter().map(|(r, _)| r).collect(),
            },
        );
    }

    /// Następne pobieranie przerwie się po `bytes` bajtach (jednorazowo).
    pub fn cut_next_after(&self, bytes: u64) {
        self.lock().cut_next_after = Some(bytes);
    }

    /// Brak sieci (każde żądanie kończy się błędem sieci).
    pub fn set_offline(&self, offline: bool) {
        self.lock().offline = offline;
    }

    /// Żądania pobrania: (adres, od którego bajtu).
    pub fn requests(&self) -> Vec<(String, u64)> {
        self.lock().requests.clone()
    }
}

fn io(e: std::io::Error) -> UpdaterError {
    UpdaterError::io(e)
}

#[async_trait]
impl ReleaseFeed for FakeFeed {
    async fn manifest(&self, channel: Channel) -> Result<ReleaseManifest, UpdaterError> {
        let st = self.lock();
        if st.offline {
            return Err(UpdaterError::network("brak sieci (atrapa)"));
        }
        let manifest = st
            .manifests
            .get(channel.as_str())
            .cloned()
            .ok_or_else(|| UpdaterError::network("HTTP 404 (atrapa)"))?;
        validate_manifest(&manifest, channel)?;
        Ok(manifest)
    }

    async fn download(
        &self,
        release: &Release,
        dest: &Path,
        progress: &(dyn Fn(DownloadProgress) + Send + Sync),
    ) -> Result<(), UpdaterError> {
        let offset = std::fs::metadata(dest).map_or(0, |m| m.len());
        let (data, cut) = {
            let mut st = self.lock();
            st.requests.push((release.url.clone(), offset));
            if st.offline {
                return Err(UpdaterError::network("brak sieci (atrapa)"));
            }
            let data = st
                .packages
                .get(&release.url)
                .cloned()
                .ok_or_else(|| UpdaterError::network("HTTP 404 (atrapa)"))?;
            (data, st.cut_next_after.take())
        };
        let total = data.len() as u64;
        let start = offset.min(total);
        let end = cut.map_or(total, |c| start.saturating_add(c).min(total));
        let from = usize::try_from(start).unwrap_or(usize::MAX);
        let to = usize::try_from(end).unwrap_or(usize::MAX);
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(dest)
            .map_err(io)?;
        file.write_all(data.get(from..to).unwrap_or_default())
            .map_err(io)?;
        progress(DownloadProgress {
            downloaded: end,
            total: Some(total),
            resumed: start > 0,
        });
        if end < total {
            return Err(UpdaterError::network(format!(
                "przerwano po {end} z {total} B (atrapa)"
            )));
        }
        Ok(())
    }
}
