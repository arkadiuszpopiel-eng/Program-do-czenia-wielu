//! Adaptery portów: pliki w dozwolonych korzeniach (bez utraty danych), `core-config`
//! z porównaj-i-zamień i origin = diagnostician, rewizje, kontekst katalogów.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use core_config_contract::{ConfigKey, ConfigLayer, ConfigStore, MachineId, Origin, Scope};
use core_config_fake::FakeConfigStore;
use diagnostician_contract::{Detection, RepairContext, RepairEnv, RepairStep};
use diagnostician_impl::{
    DiagDirs, DirContext, DownloadQueue, EntryStore, FilePort, HealthProbe, LocalFiles,
    ModuleRestarter, PortsEnv,
};
use serde_json::json;
use watchdog_contract::{ConfigHistory, ManualClock};

#[test]
fn local_files_stay_inside_roots_and_never_lose_data() {
    let root = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    let files = LocalFiles::new([root.path().to_path_buf()]).unwrap();
    let p = |s: &str| root.path().join(s).to_string_lossy().into_owned();
    std::fs::write(p("a.toml"), b"x = 1").unwrap();
    std::fs::create_dir_all(root.path().join("kopie")).unwrap();
    std::fs::write(p("kopie/a.toml"), b"x = 0").unwrap();
    files
        .move_path(&p("a.toml"), &p("kwarantanna/1/a.toml"))
        .unwrap();
    files.copy_file(&p("kopie/a.toml"), &p("a.toml")).unwrap();
    assert!(
        files.copy_file(&p("kopie/a.toml"), &p("a.toml")).is_err(),
        "bez nadpisywania"
    );
    std::fs::write(p("b.toml"), b"inna").unwrap();
    assert!(
        files
            .remove_identical_copy(&p("b.toml"), &p("kopie/a.toml"))
            .is_err(),
        "tylko identyczna kopia"
    );
    files
        .remove_identical_copy(&p("a.toml"), &p("kopie/a.toml"))
        .unwrap();
    files
        .move_path(&p("kwarantanna/1/a.toml"), &p("a.toml"))
        .unwrap();
    assert_eq!(std::fs::read(p("a.toml")).unwrap(), b"x = 1");
    let out = outside.path().join("x").to_string_lossy().into_owned();
    assert!(files.move_path(&p("a.toml"), &out).is_err());
    assert!(
        files
            .move_path(&format!("{}/../x", root.path().display()), &p("y"))
            .is_err()
    );
    assert!(files.move_path("wzgledna", &p("y")).is_err());
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(outside.path(), root.path().join("link")).unwrap();
        assert!(
            files.move_path(&p("a.toml"), &p("link/a.toml")).is_err(),
            "dowiązanie poza korzeń"
        );
    }
}

#[derive(Default)]
struct Rec(Mutex<Vec<String>>);

impl Rec {
    fn push(&self, s: String) -> Result<(), String> {
        self.0.lock().unwrap().push(s);
        Ok(())
    }
}

#[async_trait]
impl ModuleRestarter for Rec {
    async fn restart(&self, module: &str) -> Result<(), String> {
        self.push(format!("restart {module}"))
    }
}

impl EntryStore for Rec {
    fn archive(&self, store: &str, n: u64, _: &str) -> Result<(), String> {
        self.push(format!("archive {store} {n}"))
    }
    fn restore(&self, store: &str, n: u64, _: &str) -> Result<(), String> {
        self.push(format!("restore {store} {n}"))
    }
}

impl DownloadQueue for Rec {
    fn queue(&self, item: &str, _: &str) -> Result<(), String> {
        self.push(format!("queue {item}"))
    }
    fn cancel(&self, item: &str) -> Result<(), String> {
        self.push(format!("cancel {item}"))
    }
}

#[async_trait]
impl HealthProbe for Rec {
    async fn healthy(&self, _: &Detection) -> Result<bool, String> {
        Ok(true)
    }
}

impl FilePort for Rec {
    fn move_path(&self, a: &str, b: &str) -> Result<(), String> {
        self.push(format!("move {a} {b}"))
    }
    fn copy_file(&self, a: &str, b: &str) -> Result<(), String> {
        self.push(format!("copy {a} {b}"))
    }
    fn remove_identical_copy(&self, a: &str, b: &str) -> Result<(), String> {
        self.push(format!("remove {a} {b}"))
    }
}

struct History(Mutex<String>);

impl ConfigHistory for History {
    fn current_revision(&self) -> Option<String> {
        Some(self.0.lock().unwrap().clone())
    }
    fn rollback_to(&self, revision: &str) -> Result<(), String> {
        *self.0.lock().unwrap() = revision.into();
        Ok(())
    }
}

#[tokio::test]
async fn ports_env_uses_config_cas_and_narrow_ports() {
    let store = Arc::new(FakeConfigStore::new(MachineId::new("test")));
    let key = ConfigKey::new("voice.stt.device").unwrap();
    store
        .set(
            &key,
            Some(json!("vulkan")),
            &Scope::Global,
            &ConfigLayer::Shared,
            Origin::User,
        )
        .await
        .unwrap();
    let rec = Arc::new(Rec::default());
    let env = PortsEnv {
        config: store.clone(),
        history: Arc::new(History(Mutex::new("r2".into()))),
        files: rec.clone(),
        modules: rec.clone(),
        entries: rec.clone(),
        downloads: rec.clone(),
        probe: rec.clone(),
    };
    let set = RepairStep::SetConfig {
        key: "voice.stt.device".into(),
        old: Some(json!("vulkan")),
        new: Some(json!("cpu")),
    };
    env.apply(&set).await.unwrap();
    assert_eq!(
        store.history().last().unwrap().origin,
        Origin::Module("diagnostician".into())
    );
    assert!(
        env.apply(&set).await.is_err(),
        "porównaj-i-zamień: wartość już inna"
    );
    for bad in [
        "kernel.egress.allow",
        "diagnostician.autonomy",
        "improver.repeats",
    ] {
        let step = RepairStep::SetConfig {
            key: bad.into(),
            old: None,
            new: Some(json!(1)),
        };
        assert!(env.apply(&step).await.is_err(), "{bad}");
    }
    let rb = RepairStep::RollbackConfig {
        from_revision: "r2".into(),
        to_revision: "r1".into(),
    };
    env.apply(&rb).await.unwrap();
    assert!(env.apply(&rb).await.is_err(), "rewizja już inna");
    for step in [
        RepairStep::RestartModule {
            module: "voice-stt".into(),
        },
        RepairStep::ArchiveEntries {
            store: "undo-journal".into(),
            entries: 5,
            archive: "a".into(),
        },
        RepairStep::QueueDownload {
            item: "m".into(),
            sha256: "0".repeat(64),
        },
        RepairStep::MoveFile {
            from: "/a".into(),
            to: "/b".into(),
        },
    ] {
        env.apply(&step).await.unwrap();
        env.apply(&step.inverse()).await.unwrap();
    }
    assert_eq!(rec.0.lock().unwrap().len(), 8);
}

#[test]
fn dir_context_backups_quarantine_ports_and_kernel_roots() {
    let root = tempfile::tempdir().unwrap();
    let data = root.path().join("Alfa");
    std::fs::create_dir_all(data.join("kopie/config")).unwrap();
    std::fs::write(data.join("kopie/config/shared.toml"), b"").unwrap();
    let dirs = DiagDirs {
        data_root: data.clone(),
        backups: data.join("kopie"),
        quarantine: data.join("kwarantanna"),
        archive: data.join("archiwum"),
    };
    let ctx = DirContext::new(
        Arc::new(ManualClock::new(7)),
        dirs,
        &[data.join("versions")],
    );
    let shared = data
        .join("config/shared.toml")
        .to_string_lossy()
        .into_owned();
    assert!(ctx.latest_backup(&shared).is_some());
    assert!(
        ctx.latest_backup(&data.join("config/inne.toml").to_string_lossy())
            .is_none()
    );
    assert_ne!(ctx.quarantine_path(&shared), ctx.quarantine_path(&shared));
    let port = ctx.free_port(40_000).unwrap();
    assert!(std::net::TcpListener::bind(("127.0.0.1", port)).is_ok());
    assert!(ctx.is_kernel_path(&data.join("versions/0.0.2.zip").to_string_lossy()));
    assert!(!ctx.is_kernel_path(&data.join("versions2/x").to_string_lossy()));
    assert_eq!(ctx.config("x"), None);
}
