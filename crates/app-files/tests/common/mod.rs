//! Środowisko testów `app-files`: katalogi tymczasowe, sesje i artefakty z atrap, powłoka bez okien
//! (odpowiedzi dialogów z kolejki), schowek i sygnały systemowe z atrap, prawdziwy `transfer-impl`.

#![allow(dead_code, clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use accounts_hub_contract::SecretStore;
use accounts_hub_fake::MemorySecretStore;
use app_api::paths::AppPaths;
use app_api::ports::{HeadlessShell, ShellPort};
use app_files::{ArtifactDocuments, FilesApp, FilesDeps};
use artifacts_contract::Artifacts;
use artifacts_fake::FakeArtifacts;
use platform_contract::{
    ClipboardContent, ClipboardPort, PlatformError, PowerSnapshot, PowerSource, SignalEvent,
    SystemSignals, SystemSignalsPort,
};
use sessions_contract::{NewSession, SessionCatalog, SessionId, Sessions};
use sessions_fake::FakeSessions;
use tempfile::TempDir;
use transfer_contract::{
    Category, DocumentStore, Limits, MachineInfo, SystemClock, Transfer, TransferPorts,
};
use transfer_impl::{DirDocumentStore, DirFilter, KdfParams, TransferConfig, UuidIds, ZipTransfer};

pub const MACHINE: &str = "0123456789abcdef0123456789abcdef";

/// Schowek z atrapy.
#[derive(Default)]
pub struct TestClipboard(pub Mutex<Option<ClipboardContent>>);

impl ClipboardPort for TestClipboard {
    fn get(&self) -> Result<ClipboardContent, PlatformError> {
        Ok(self
            .0
            .lock()
            .unwrap()
            .clone()
            .unwrap_or(ClipboardContent::Empty))
    }
    fn set(&self, content: ClipboardContent) -> Result<(), PlatformError> {
        *self.0.lock().unwrap() = Some(content);
        Ok(())
    }
    fn restore_previous(&self) -> Result<bool, PlatformError> {
        Ok(false)
    }
}

/// Sygnały z atrapy (bateria / pełny ekran przełączane w teście).
#[derive(Default)]
pub struct TestSignals(pub Mutex<SystemSignals>);

impl TestSignals {
    pub fn on_battery(&self, battery: bool) {
        self.0.lock().unwrap().power = PowerSnapshot {
            source: if battery {
                PowerSource::Battery
            } else {
                PowerSource::Ac
            },
            battery_present: true,
            ..PowerSnapshot::UNKNOWN
        };
    }
}

impl SystemSignalsPort for TestSignals {
    fn snapshot(&self) -> SystemSignals {
        self.0.lock().unwrap().clone()
    }
    fn drain_events(&self) -> Vec<SignalEvent> {
        Vec::new()
    }
    fn wait_events(&self, _t: std::time::Duration) -> Vec<SignalEvent> {
        Vec::new()
    }
}

/// Szybkie parametry Argon2id.
pub fn fast_kdf() -> KdfParams {
    KdfParams {
        m_kib: 64,
        t: 1,
        p: 1,
    }
}

pub struct Env {
    pub dir: TempDir,
    pub paths: AppPaths,
    pub sessions: Arc<FakeSessions>,
    pub artifacts: Arc<FakeArtifacts>,
    pub shell: Arc<HeadlessShell>,
    pub clipboard: Arc<TestClipboard>,
    pub signals: Arc<TestSignals>,
    pub secrets: Arc<MemorySecretStore>,
    pub transfer: Arc<ZipTransfer>,
    pub config: Arc<DirDocumentStore>,
    pub docs: Arc<ArtifactDocuments>,
    pub app: Arc<FilesApp>,
}

impl Env {
    pub fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let paths = AppPaths::under(dir.path());
        let sessions = Arc::new(FakeSessions::new());
        let artifacts = Arc::new(FakeArtifacts::new(paths.user_root.clone()));
        let shell = Arc::new(HeadlessShell::default());
        let clipboard = Arc::new(TestClipboard::default());
        let signals = Arc::new(TestSignals::default());
        let secrets = Arc::new(MemorySecretStore::new());
        let config = Arc::new(DirDocumentStore::new(
            paths.config.clone(),
            DirFilter::flat(&["toml"]),
        ));
        let docs = ArtifactDocuments::new(paths.workdirs().join("Import"), 1 << 20);
        docs.bind(
            artifacts.clone() as Arc<dyn Artifacts>,
            sessions.clone() as Arc<dyn Sessions>,
        );
        let transfer = Arc::new(transfer(
            &dir,
            &sessions,
            &secrets,
            &config,
            &docs,
            "snapshots",
        ));
        let app = FilesApp::open(FilesDeps {
            paths: paths.clone(),
            sessions: sessions.clone(),
            artifacts: artifacts.clone(),
            shell: shell.clone() as Arc<dyn ShellPort>,
            clipboard: Some(clipboard.clone()),
            transfer: Some(transfer.clone() as Arc<dyn Transfer>),
            secrets: Some(secrets.clone() as Arc<dyn SecretStore>),
            signals: Some(signals.clone()),
            events: None,
            docs: Some(docs.clone()),
        });
        Self {
            dir,
            paths,
            sessions,
            artifacts,
            shell,
            clipboard,
            signals,
            secrets,
            transfer,
            config,
            docs,
            app,
        }
    }

    /// Nowa sesja z katalogiem w `user/Sesje/<nazwa>`.
    pub fn session(&self, name: &str) -> SessionId {
        let workdir = self.paths.workdirs().join(name);
        std::fs::create_dir_all(&workdir).unwrap();
        self.sessions
            .create_session(NewSession {
                title: name.into(),
                workdir: Some(workdir),
                ..NewSession::default()
            })
            .unwrap()
            .id
    }

    /// Plik użytkownika poza danymi Alfy.
    pub fn file(&self, name: &str, bytes: &[u8]) -> PathBuf {
        let path = self.dir.path().join("Dokumenty").join(name);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, bytes).unwrap();
        path
    }
}

/// `ZipTransfer` nad sesjami, sekretami, konfiguracją i artefaktami (snapshoty w `snapshots`).
pub fn transfer(
    dir: &TempDir,
    sessions: &Arc<FakeSessions>,
    secrets: &Arc<MemorySecretStore>,
    config: &Arc<DirDocumentStore>,
    docs: &Arc<ArtifactDocuments>,
    snapshots: &str,
) -> ZipTransfer {
    let mut documents: BTreeMap<Category, Arc<dyn DocumentStore>> = BTreeMap::new();
    documents.insert(Category::ConfigCommon, config.clone());
    documents.insert(Category::Artifacts, docs.clone());
    let ports = TransferPorts {
        sessions: Some(sessions.clone() as Arc<dyn Sessions>),
        documents,
        secrets: Some(secrets.clone() as Arc<dyn SecretStore>),
        machine: MachineInfo {
            id: MACHINE.into(),
            name: "desktop".into(),
            os: "Windows 11".into(),
            hw_class: "desktop-amd".into(),
        },
        app_version: semver::Version::new(0, 0, 1),
        workdir_root: None,
        clock: Arc::new(SystemClock),
        ids: Arc::new(UuidIds),
        limits: Limits::default(),
    };
    let mut cfg = TransferConfig::new(dir.path().join(snapshots));
    cfg.kdf = fast_kdf();
    ZipTransfer::new(ports, cfg).unwrap()
}

pub const PNG: &[u8] =
    b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR\0\0\0\x01\0\0\0\x01\x08\x06\0\0\0\x1f\x15\xc4\x89";

pub fn exists(p: &Path) -> bool {
    p.exists()
}
