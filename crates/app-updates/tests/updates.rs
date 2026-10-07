//! Aktualizacje w aplikacji: komendy `updates_*` na prawdziwym module plików (katalog
//! tymczasowy), atrapie źródła wydań i paczkach podpisanych kluczem z testu; tryby
//! automatycznie/pytaj/ręcznie, zdarzenia `UpdateStatus`, restart przez launcher z blokadą
//! w trakcie pracy, „Przywróć poprzednią wersję”, „Co nowego”, „O programie”, `mark_good`.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::io::{Cursor, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use app_api::AppError;
use app_api::dto::{AlfaEvent, LocalizedText, UpdatePhase};
use app_api::events::EventHub;
use app_api::ports::HeadlessShell;
use app_updates::{LauncherPort, Schedule, UpdatesApp, UpdatesDeps, keys};
use core_config_contract::{ConfigKey, MachineId};
use core_config_fake::FakeConfigStore;
use semver::Version;
use sha2::Digest;
use updater_contract::{Channel, Release, ReleaseFeed, Updater};
use updater_fake::FakeFeed;
use updater_impl::{FsUpdater, ServiceOptions, UpdaterConfig};

fn v(s: &str) -> Version {
    Version::parse(s).unwrap()
}

#[derive(Default)]
struct Launcher(Mutex<Vec<PathBuf>>);

impl LauncherPort for Launcher {
    fn restart(&self, launcher: &Path) -> Result<(), AppError> {
        self.0.lock().unwrap().push(launcher.to_path_buf());
        Ok(())
    }
}

struct Env {
    _dir: tempfile::TempDir,
    keys: minisign::KeyPair,
    updater: Arc<FsUpdater>,
    feed: Arc<FakeFeed>,
    shell: Arc<HeadlessShell>,
    launcher: Arc<Launcher>,
}

fn package(version: &str) -> Vec<u8> {
    let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let opts = zip::write::SimpleFileOptions::default();
    for (name, data) in [
        ("alfa-desktop.exe", "MZ".to_owned()),
        ("version.json", format!("{{\"version\":\"{version}\"}}")),
        ("notes.md", format!("Nowości {version}")),
    ] {
        zip.start_file(name, opts).unwrap();
        zip.write_all(data.as_bytes()).unwrap();
    }
    zip.finish().unwrap().into_inner()
}

impl Env {
    fn new(running: &str) -> Self {
        let dir = tempfile::tempdir().unwrap();
        let keys = minisign::KeyPair::generate_unencrypted_keypair().unwrap();
        let mut config = UpdaterConfig::new(&dir.path().join("Alfa"));
        config.public_key = Some(keys.pk.to_base64());
        let updater = Arc::new(FsUpdater::new(config).unwrap());
        let vdir = updater.layout().version_dir(&v(running));
        std::fs::create_dir_all(&vdir).unwrap();
        std::fs::write(vdir.join("alfa-desktop.exe"), b"MZ").unwrap();
        updater.switch_to(&v(running)).unwrap();
        Self {
            _dir: dir,
            keys,
            updater,
            feed: Arc::new(FakeFeed::new()),
            shell: Arc::new(HeadlessShell::default()),
            launcher: Arc::default(),
        }
    }

    fn publish(&self, version: &str) {
        let bytes = package(version);
        let trusted = format!("timestamp:1\tversion:{version}");
        let signature = minisign::sign(
            Some(&self.keys.pk),
            &self.keys.sk,
            Cursor::new(&bytes),
            Some(&trusted),
            None,
        )
        .unwrap()
        .to_string();
        let release = Release {
            version: v(version),
            url: format!("alfa-{version}.zip"),
            sha256: sha2::Sha256::digest(&bytes)
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect(),
            minisign: signature,
            notes: format!("Co nowego w {version}"),
            min_previous: None,
        };
        self.feed.publish(Channel::Stable, vec![(release, bytes)]);
    }

    fn app(
        &self,
        running: &str,
        settings: &[(&str, serde_json::Value)],
        events: Option<EventHub>,
    ) -> Arc<UpdatesApp> {
        let defaults = settings
            .iter()
            .map(|(k, val)| (ConfigKey::new(*k).unwrap(), val.clone()));
        let config = FakeConfigStore::new(MachineId::new("test")).with_defaults(defaults);
        UpdatesApp::open(UpdatesDeps {
            updater: self.updater.clone(),
            feed: Some(self.feed.clone() as Arc<dyn ReleaseFeed>),
            config: Arc::new(config),
            events,
            shell: self.shell.clone(),
            version: running.to_owned(),
            launcher: Some(self.launcher.clone()),
            options: ServiceOptions {
                max_resumes: 1,
                retry_delay: Duration::ZERO,
            },
        })
    }
}

async fn wait_phase(app: &UpdatesApp, phase: UpdatePhase) {
    for _ in 0..500 {
        if app.status().await.unwrap().phase == phase {
            return;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    panic!("brak etapu {phase:?}: {:?}", app.status().await.unwrap());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn ask_mode_check_download_restart() {
    let env = Env::new("1.0.0");
    env.publish("1.1.0");
    let hub = EventHub::start(Duration::from_millis(1));
    let mut rx = hub.subscribe();
    let app = env.app("1.0.0", &[(keys::MODE, "ask".into())], Some(hub));
    let view = app.status().await.unwrap();
    assert_eq!(
        (view.phase, view.current.as_str()),
        (UpdatePhase::Idle, "1.0.0")
    );
    let err = app.download().await.unwrap_err();
    assert_eq!(
        err.code,
        app_api::ErrorCode::InvalidInput,
        "najpierw sprawdź"
    );
    let view = app.check().await.unwrap();
    assert_eq!(view.phase, UpdatePhase::Available);
    assert_eq!(view.available.unwrap().notes, "Co nowego w 1.1.0");
    app.download().await.unwrap();
    wait_phase(&app, UpdatePhase::Ready).await;
    let view = app.status().await.unwrap();
    assert_eq!(
        (view.ready.as_deref(), view.previous.as_deref()),
        (Some("1.1.0"), Some("1.0.0"))
    );
    // Zdarzenia dla UI: postęp aż do gotowości.
    let mut phases = Vec::new();
    while let Ok(batch) = rx.try_recv() {
        for e in batch.iter() {
            if let AlfaEvent::UpdateStatus { status } = e {
                phases.push(status.phase);
            }
        }
    }
    assert!(phases.contains(&UpdatePhase::Downloading), "{phases:?}");
    assert_eq!(phases.last(), Some(&UpdatePhase::Ready));

    // Trwa zadanie agentki — restart zablokowany.
    let busy = LocalizedText::new("Agentka wykonuje zadanie.", "An agent is working.");
    let reason = busy.clone();
    app.set_busy_probe(Arc::new(move || {
        let r = reason.clone();
        Box::pin(async move { Some(r) })
    }));
    assert_eq!(app.status().await.unwrap().restart_blocked, Some(busy));
    let err = app.restart().await.unwrap_err();
    assert_eq!(err.code, app_api::ErrorCode::Forbidden);
    assert!(env.launcher.0.lock().unwrap().is_empty());
    app.set_busy_probe(Arc::new(|| Box::pin(async { None })));
    app.restart().await.unwrap();
    assert_eq!(
        *env.launcher.0.lock().unwrap(),
        vec![env.updater.layout().launcher.clone()]
    );
    assert_eq!(env.shell.calls(), vec!["exit_app".to_owned()]);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn scheduled_checks_follow_mode() {
    let env = Env::new("1.0.0");
    env.publish("1.1.0");
    let day = Duration::from_secs(24 * 3600);
    let manual = env.app("1.0.0", &[(keys::MODE, "manual".into())], None);
    assert!(!manual.scheduled_check(day).await, "ręcznie — nigdy samo");
    drop(manual);
    let ask = env.app("1.0.0", &[(keys::MODE, "ask".into())], None);
    assert!(ask.scheduled_check(day).await);
    assert_eq!(ask.status().await.unwrap().phase, UpdatePhase::Available);
    assert!(!ask.scheduled_check(day).await, "raz na dobę");
    drop(ask);
    let auto = env.app("1.0.0", &[(keys::MODE, "auto".into())], None);
    assert!(auto.scheduled_check(Duration::ZERO).await);
    let view = auto.status().await.unwrap();
    assert_eq!(
        (view.phase, view.ready.as_deref()),
        (UpdatePhase::Ready, Some("1.1.0"))
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn rollback_whats_new_about_and_mark_good() {
    let env = Env::new("1.0.0");
    env.publish("1.1.0");
    let old = env.app("1.0.0", &[(keys::MODE, "auto".into())], None);
    assert_eq!(
        old.whats_new().await.unwrap(),
        None,
        "pierwsze uruchomienie"
    );
    old.scheduled_check(Duration::ZERO).await;
    drop(old);
    // Restart: działa 1.1.0 (launcher przełączył) — „Co nowego” raz, potem mark_good.
    let app = env.app("1.1.0", &[], None);
    let news = app.whats_new().await.unwrap().unwrap();
    assert_eq!(
        (news.version.as_str(), news.notes.as_str()),
        ("1.1.0", "Nowości 1.1.0")
    );
    app.dismiss_whats_new().await.unwrap();
    assert_eq!(app.whats_new().await.unwrap(), None);
    app.spawn_background(Schedule {
        healthy_after: Duration::from_millis(20),
        first_check_after: Duration::from_secs(3600),
        tick: Duration::from_secs(3600),
        check_every: Duration::from_secs(3600),
    });
    for _ in 0..200 {
        if !env.updater.state().unwrap().unwrap().pending {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    assert!(!env.updater.state().unwrap().unwrap().pending, "mark_good");
    // „Przywróć poprzednią wersję” → aktywna 1.0.0 od restartu.
    let view = app.rollback().await.unwrap();
    assert_eq!(
        (view.phase, view.ready.as_deref()),
        (UpdatePhase::Ready, Some("1.0.0"))
    );
    let about = app.about().await.unwrap();
    assert_eq!(about.version, "1.1.0");
    assert!(
        !about.updates_configured,
        "brak wbudowanego adresu wydań w teście"
    );
    let (_, licenses) = app_updates::licenses();
    assert_eq!(about.licenses, licenses);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn whats_new_can_be_turned_off_and_disabled_build() {
    let env = Env::new("1.0.0");
    let first = env.app("1.0.0", &[], None);
    assert_eq!(first.whats_new().await.unwrap(), None);
    drop(first);
    let vdir = env.updater.layout().version_dir(&v("1.2.0"));
    std::fs::create_dir_all(&vdir).unwrap();
    std::fs::write(vdir.join("alfa-desktop.exe"), b"MZ").unwrap();
    let off = env.app("1.2.0", &[(keys::WHATS_NEW, false.into())], None);
    assert_eq!(off.whats_new().await.unwrap(), None);
    drop(off);
    let on = env.app("1.2.0", &[], None);
    assert_eq!(
        on.whats_new().await.unwrap(),
        None,
        "wyłączone = oznaczone jako widziane"
    );
    // Build bez adresu wydań: aktualizacje wyłączone, „Sprawdź teraz” = niedostępne.
    let disabled = UpdatesApp::open(UpdatesDeps {
        updater: env.updater.clone(),
        feed: None,
        config: Arc::new(FakeConfigStore::new(MachineId::new("t"))),
        events: None,
        shell: env.shell.clone(),
        version: "1.0.0".into(),
        launcher: Some(env.launcher.clone()),
        options: ServiceOptions::default(),
    });
    assert_eq!(
        disabled.status().await.unwrap().phase,
        UpdatePhase::Disabled
    );
    let err = disabled.check().await.unwrap_err();
    assert_eq!(err.code, app_api::ErrorCode::Unavailable);
    let err = disabled.restart().await.unwrap_err();
    assert_eq!(err.code, app_api::ErrorCode::InvalidInput);
}
