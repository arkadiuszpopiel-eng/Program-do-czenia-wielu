//! Środowisko testów `updater-impl`: katalog tymczasowy jako `%LOCALAPPDATA%\Alfa`, para kluczy
//! minisign wygenerowana w teście.

#![allow(dead_code, clippy::unwrap_used, clippy::expect_used)]

use std::io::Cursor;
use std::path::PathBuf;

use sha2::Digest;
use tempfile::TempDir;
use updater_contract::contract_tests::{Fixture, Harness};
use updater_contract::{Release, Updater};
use updater_impl::{FsUpdater, UpdaterConfig};

pub struct H {
    pub dir: TempDir,
    pub updater: FsUpdater,
    pub keys: minisign::KeyPair,
    pub other: minisign::KeyPair,
}

pub fn sign(keys: &minisign::KeyPair, data: &[u8], trusted: &str) -> String {
    minisign::sign(
        Some(&keys.pk),
        &keys.sk,
        Cursor::new(data),
        Some(trusted),
        None,
    )
    .unwrap()
    .to_string()
}

pub fn sha(data: &[u8]) -> String {
    sha2::Sha256::digest(data)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

pub fn harness() -> H {
    let dir = tempfile::tempdir().unwrap();
    let keys = minisign::KeyPair::generate_unencrypted_keypair().unwrap();
    let other = minisign::KeyPair::generate_unencrypted_keypair().unwrap();
    let mut config = UpdaterConfig::new(&dir.path().join("Alfa"));
    config.public_key = Some(keys.pk.to_base64());
    let updater = FsUpdater::new(config).unwrap();
    H {
        dir,
        updater,
        keys,
        other,
    }
}

impl Harness for H {
    fn updater(&self) -> &dyn Updater {
        &self.updater
    }

    fn install(&self, version: &str) {
        let dir = self.updater.layout().versions.join(version);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("alfa-desktop.exe"), b"MZ").unwrap();
        std::fs::write(
            dir.join("version.json"),
            format!("{{\"version\":\"{version}\"}}"),
        )
        .unwrap();
    }

    fn corrupt(&self, version: &str) {
        let dir = self.updater.layout().versions.join(version);
        let _ = std::fs::remove_file(dir.join("alfa-desktop.exe"));
    }

    fn release(&self, version: &str, fixture: Fixture) -> (Release, PathBuf) {
        let data = format!("paczka Alfy {version}").into_bytes();
        let path = self
            .dir
            .path()
            .join(format!("alfa-{version}-{fixture:?}.zip"));
        std::fs::write(&path, &data).unwrap();
        let tag = format!("timestamp:1\tfile:alfa-{version}.zip\tversion:{version}");
        let (signature, digest) = match fixture {
            Fixture::Good => (sign(&self.keys, &data, &tag), sha(&data)),
            Fixture::BadSignature => (sign(&self.keys, b"inna tresc", &tag), sha(&data)),
            Fixture::BadHash => (sign(&self.keys, &data, &tag), sha(b"inna tresc")),
            Fixture::WrongVersionTag => (
                sign(&self.keys, &data, "timestamp:1\tversion:0.0.1"),
                sha(&data),
            ),
            Fixture::OtherKey => (sign(&self.other, &data, &tag), sha(&data)),
        };
        let release = Release {
            version: semver::Version::parse(version).unwrap(),
            url: format!("https://repo.example/alfa-{version}.zip"),
            sha256: digest,
            minisign: signature,
            notes: "Co nowego: poprawki".into(),
            min_previous: None,
        };
        (release, path)
    }
}
