//! Testy atrapy: kontrakt współdzielony, skryptowane błędy, historia przełączeń.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::PathBuf;

use semver::Version;
use updater_contract::contract_tests::{self, Fixture, Harness};
use updater_contract::{Release, Updater, UpdaterError};
use updater_fake::{FAKE_KEY, FakeUpdater, fake_signature, sha256_hex};

struct H(FakeUpdater);

fn v(s: &str) -> Version {
    Version::parse(s).unwrap()
}

impl Harness for H {
    fn updater(&self) -> &dyn Updater {
        &self.0
    }
    fn install(&self, version: &str) {
        self.0.install(&v(version));
    }
    fn corrupt(&self, version: &str) {
        self.0.corrupt(&v(version));
    }
    fn release(&self, version: &str, fixture: Fixture) -> (Release, PathBuf) {
        let data = format!("paczka {version}").into_bytes();
        let path = PathBuf::from(format!("/repo/{version}-{fixture:?}.zip"));
        self.0.add_package(&path, &data);
        let mut r = FakeUpdater::signed_release(&v(version), &data);
        let tag = format!("version:{version}");
        match fixture {
            Fixture::Good => {}
            Fixture::BadSignature => r.minisign = fake_signature(FAKE_KEY, &tag, b"inna"),
            Fixture::BadHash => r.sha256 = sha256_hex(b"inna"),
            Fixture::WrongVersionTag => {
                r.minisign = fake_signature(FAKE_KEY, "version:0.0.1", &data)
            }
            Fixture::OtherKey => r.minisign = fake_signature("inny", &tag, &data),
        }
        (r, path)
    }
}

#[test]
fn contract_suite() {
    contract_tests::run_all(|| H(FakeUpdater::new()));
}

#[test]
fn scripted_failure_and_switch_history() {
    let f = FakeUpdater::new();
    f.install(&v("1.0.0"));
    f.install(&v("1.1.0"));
    f.fail_next(UpdaterError::io("dysk"));
    assert_eq!(f.switch_to(&v("1.0.0")), Err(UpdaterError::io("dysk")));
    f.switch_to(&v("1.0.0")).unwrap();
    f.switch_to(&v("1.1.0")).unwrap();
    f.rollback().unwrap();
    assert_eq!(f.switches(), vec![v("1.0.0"), v("1.1.0"), v("1.0.0")]);
    assert!(
        f.verify_release(
            &FakeUpdater::signed_release(&v("2.0.0"), b"x"),
            &PathBuf::from("/brak")
        )
        .is_err()
    );
}
