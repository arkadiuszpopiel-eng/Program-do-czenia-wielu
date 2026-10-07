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

#[tokio::test]
async fn fake_feed_resumes_and_reports_requests() {
    use updater_contract::{Channel, ReleaseFeed};
    use updater_fake::FakeFeed;
    let feed = FakeFeed::new();
    let data = vec![5u8; 1000];
    let release = FakeUpdater::signed_release(&v("1.1.0"), &data);
    feed.publish(Channel::Stable, vec![(release.clone(), data.clone())]);
    assert_eq!(
        feed.manifest(Channel::Stable).await.unwrap().releases.len(),
        1
    );
    assert!(feed.manifest(Channel::Beta).await.is_err());
    let dir = tempfile::tempdir().unwrap();
    let dest = dir.path().join("p.part");
    feed.cut_next_after(300);
    let err = feed.download(&release, &dest, &|_| {}).await.unwrap_err();
    assert!(matches!(err, UpdaterError::Network { .. }));
    feed.download(&release, &dest, &|_| {}).await.unwrap();
    assert_eq!(std::fs::read(&dest).unwrap(), data);
    assert_eq!(
        feed.requests(),
        vec![(release.url.clone(), 0), (release.url.clone(), 300)]
    );
    feed.set_offline(true);
    assert!(feed.manifest(Channel::Stable).await.is_err());
}
