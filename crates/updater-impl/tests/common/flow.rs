//! Środowisko testów pełnego cyklu: katalog instalacji z uruchomioną wersją, lokalny serwer
//! HTTP z manifestem kanału i paczkami ZIP podpisanymi parą kluczy minisign z testu.

#![allow(dead_code, clippy::unwrap_used, clippy::expect_used)]

use std::io::{Cursor, Write};
use std::sync::Arc;
use std::time::Duration;

use semver::Version;
use updater_contract::{
    Channel, RELEASES_SCHEMA, Release, ReleaseFeed, ReleaseManifest, UpdateMode, Updater,
};
use updater_impl::{FsUpdater, HttpFeed, ServiceOptions, UpdateService};
use zip::write::SimpleFileOptions;

use super::http::Server;
use super::{H, harness, sha, sign};
use updater_contract::contract_tests::Harness;

pub fn v(s: &str) -> Version {
    Version::parse(s).unwrap()
}

/// Dane nieściśliwe (deterministyczny generator liniowy) — paczka ma realny rozmiar.
pub fn noise(len: usize, seed: u64) -> Vec<u8> {
    let mut x = seed.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1);
    (0..len)
        .map(|_| {
            x = x
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            (x >> 33) as u8
        })
        .collect()
}

/// Paczka wersji: `alfa-desktop.exe`, `version.json`, `notes.md` + dodatkowe wpisy.
pub fn package(version: &str, extra: &[(&str, &[u8])]) -> Vec<u8> {
    let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let opts = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    let mut add = |name: &str, data: &[u8]| {
        zip.start_file(name, opts).unwrap();
        zip.write_all(data).unwrap();
    };
    add(
        "alfa-desktop.exe",
        format!("MZ aplikacja {version}").as_bytes(),
    );
    add(
        "version.json",
        format!("{{\"version\":\"{version}\"}}").as_bytes(),
    );
    add("notes.md", format!("- Nowości w {version}").as_bytes());
    for (name, data) in extra {
        add(name, data);
    }
    zip.finish().unwrap().into_inner()
}

pub struct Flow {
    pub h: H,
    pub server: Server,
    pub updater: Arc<FsUpdater>,
    pub service: Arc<UpdateService>,
}

impl Flow {
    /// Uruchomiona wersja `running` (zainstalowana, aktywna, dobra) i serwer bez wydań.
    pub async fn new(running: &str) -> Self {
        Self::with_options(
            running,
            ServiceOptions {
                max_resumes: 3,
                retry_delay: Duration::ZERO,
            },
        )
        .await
    }

    pub async fn with_options(running: &str, options: ServiceOptions) -> Self {
        let h = harness();
        h.install(running);
        h.updater.switch_to(&v(running)).unwrap();
        h.updater.mark_good(&v(running)).unwrap();
        let server = Server::start().await;
        let updater = Arc::new(FsUpdater::new(h.updater.config().clone()).unwrap());
        let feed = HttpFeed::new(&format!("{}/feed", server.base), true).unwrap();
        let service = Arc::new(UpdateService::new(
            updater.clone(),
            Some(Arc::new(feed) as Arc<dyn ReleaseFeed>),
            v(running),
            Channel::Stable,
            UpdateMode::Ask,
            options,
        ));
        Self {
            h,
            server,
            updater,
            service,
        }
    }

    /// Wydanie podpisane kluczem z testu (komentarz zaufany z `tag` jako wersją).
    pub fn signed(&self, version: &str, tag: &str, bytes: &[u8]) -> Release {
        let trusted = format!("timestamp:1\tfile:alfa-{version}-x64.zip\tversion:{tag}");
        Release {
            version: v(version),
            url: format!("alfa-{version}-x64.zip"),
            sha256: sha(bytes),
            minisign: sign(&self.h.keys, bytes, &trusted),
            notes: format!("Co nowego w {version}"),
            min_previous: None,
        }
    }

    /// Publikuje paczkę i manifest kanału z wydaniami.
    pub fn publish(&self, channel: &str, releases: &[(Release, Vec<u8>)]) {
        for (r, bytes) in releases {
            self.server.put(&format!("/feed/{}", r.url), bytes.clone());
        }
        let manifest = ReleaseManifest {
            schema: RELEASES_SCHEMA,
            channel: channel.to_owned(),
            releases: releases.iter().map(|(r, _)| r.clone()).collect(),
        };
        self.server.put(
            &format!("/feed/{channel}.json"),
            serde_json::to_vec(&manifest).unwrap(),
        );
    }

    /// Publikuje poprawnie podpisaną paczkę wersji w kanale stabilnym.
    pub fn publish_good(&self, version: &str) -> (Release, Vec<u8>) {
        let bytes = package(version, &[]);
        let release = self.signed(version, version, &bytes);
        self.publish("stable", &[(release.clone(), bytes.clone())]);
        (release, bytes)
    }

    pub fn active(&self) -> Version {
        self.updater.state().unwrap().unwrap().active
    }

    pub fn staging_files(&self) -> Vec<String> {
        let dir = self.updater.layout().root.join("staging");
        std::fs::read_dir(dir)
            .map(|d| {
                d.flatten()
                    .map(|e| e.file_name().to_string_lossy().into_owned())
                    .collect()
            })
            .unwrap_or_default()
    }
}
