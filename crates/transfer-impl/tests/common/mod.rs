//! Środowisko testów `transfer-impl`: prawdziwy kontener ZIP i magazyny na katalogach
//! tymczasowych, sesje i sekrety z atrap (`sessions-fake`, `accounts-hub-fake`).

#![allow(dead_code, clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeMap;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use accounts_hub_contract::SecretStore;
use accounts_hub_fake::MemorySecretStore;
use sessions_contract::Sessions;
use sessions_fake::FakeSessions;
use tempfile::TempDir;
use transfer_contract::contract_tests::Harness;
use transfer_contract::{
    Category, DocumentStore, Limits, MachineInfo, SystemClock, Transfer, TransferPorts,
};
use transfer_impl::{DirDocumentStore, DirFilter, KdfParams, TransferConfig, UuidIds, ZipTransfer};

pub const MACHINE: &str = "fedcba9876543210fedcba9876543210";

pub struct H {
    pub dir: TempDir,
    pub transfer: ZipTransfer,
    pub sessions: Arc<FakeSessions>,
    pub stores: BTreeMap<Category, Arc<DirDocumentStore>>,
    pub secrets: Arc<MemorySecretStore>,
}

/// Szybkie parametry Argon2id (testy).
pub fn fast_kdf() -> KdfParams {
    KdfParams {
        m_kib: 64,
        t: 1,
        p: 1,
    }
}

pub fn filter(category: Category) -> DirFilter {
    match category {
        Category::ConfigCommon | Category::ConfigMachine => DirFilter::flat(&["toml"]),
        Category::Memory | Category::Logs => DirFilter::flat(&["ndjson"]),
        _ => DirFilter::tree(),
    }
}

pub fn harness_with(limits: Limits) -> H {
    let dir = tempfile::tempdir().unwrap();
    let sessions = Arc::new(FakeSessions::new());
    let stores: BTreeMap<Category, Arc<DirDocumentStore>> = Category::DOCUMENTS
        .into_iter()
        .map(|c| {
            (
                c,
                Arc::new(DirDocumentStore::new(
                    dir.path().join("dane").join(c.dir()),
                    filter(c),
                )),
            )
        })
        .collect();
    let secrets = Arc::new(MemorySecretStore::new());
    let ports = TransferPorts {
        sessions: Some(sessions.clone() as Arc<dyn Sessions>),
        documents: stores
            .iter()
            .map(|(c, s)| (*c, s.clone() as Arc<dyn DocumentStore>))
            .collect(),
        secrets: Some(secrets.clone() as Arc<dyn SecretStore>),
        machine: MachineInfo {
            id: MACHINE.into(),
            name: "laptop".into(),
            os: "Windows 11".into(),
            hw_class: "laptop-cuda".into(),
        },
        app_version: semver::Version::new(0, 0, 1),
        workdir_root: None,
        clock: Arc::new(SystemClock),
        ids: Arc::new(UuidIds),
        limits,
    };
    let mut config = TransferConfig::new(dir.path().join("snapshots"));
    config.kdf = fast_kdf();
    let transfer = ZipTransfer::new(ports, config).unwrap();
    H {
        dir,
        transfer,
        sessions,
        stores,
        secrets,
    }
}

pub fn harness() -> H {
    harness_with(Limits::default())
}

impl Harness for H {
    fn transfer(&self) -> &dyn Transfer {
        &self.transfer
    }
    fn sessions(&self) -> &dyn Sessions {
        self.sessions.as_ref()
    }
    fn store(&self, category: Category) -> &dyn DocumentStore {
        self.stores[&category].as_ref()
    }
    fn secrets(&self) -> &dyn SecretStore {
        self.secrets.as_ref()
    }
    fn path(&self, name: &str) -> PathBuf {
        self.dir.path().join("paczki").join(name)
    }
    fn entries(&self, package: &Path) -> Vec<(String, Vec<u8>)> {
        let file = std::fs::File::open(package).unwrap();
        let mut zip = zip::ZipArchive::new(file).unwrap();
        (0..zip.len())
            .map(|i| {
                let mut f = zip.by_index(i).unwrap();
                let mut bytes = Vec::new();
                f.read_to_end(&mut bytes).unwrap();
                (f.name().to_owned(), bytes)
            })
            .collect()
    }
    fn packages_in(&self, dir: &Path) -> Vec<PathBuf> {
        let mut out: Vec<PathBuf> = std::fs::read_dir(dir)
            .map(|e| {
                e.flatten()
                    .map(|e| e.path())
                    .filter(|p| p.extension().is_some_and(|x| x == "alfa"))
                    .collect()
            })
            .unwrap_or_default();
        out.sort();
        out
    }
    fn machine_id(&self) -> String {
        MACHINE.to_owned()
    }
}

/// Wszystkie pliki pod katalogiem (ścieżki względne), posortowane.
pub fn files_under(root: &Path) -> Vec<PathBuf> {
    fn walk(dir: &Path, root: &Path, out: &mut Vec<PathBuf>) {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for e in entries.flatten() {
                let p = e.path();
                if p.is_dir() {
                    walk(&p, root, out);
                } else {
                    out.push(p.strip_prefix(root).unwrap().to_path_buf());
                }
            }
        }
    }
    let mut out = Vec::new();
    walk(root, root, &mut out);
    out.sort();
    out
}

/// Buduje paczkę „ręcznie” (dowolne nazwy wpisów, dowolny manifest) — do testów odporności.
/// `manifest = None` → manifest v1 wyliczony z wpisów; wpisy w podanej kolejności po manifeście.
pub fn craft(path: &Path, manifest: Option<serde_json::Value>, entries: &[(&str, Vec<u8>)]) {
    use std::io::Write;
    let manifest = manifest.unwrap_or_else(|| {
        let content: Vec<transfer_contract::ContentEntry> = entries
            .iter()
            .map(|(p, b)| transfer_contract::ContentEntry::of(p, b))
            .collect();
        serde_json::json!({
            "schema_version": "1.0.0",
            "app_version": "0.0.1",
            "kind": "export",
            "created_at": "2026-09-30T12:00:00Z",
            "source_machine": { "id": MACHINE, "name": "x", "os": "Windows 11", "hw_class": "laptop-cuda" },
            "scope": { "keys": [], "sessions": [], "counts": { "sessions": 0, "turns": 0, "documents": 0, "memory_entries": 0, "artifacts": 0, "secrets": 0 } },
            "content_sha256": transfer_contract::content_sha256(&content),
            "content": content,
            "encryption": null,
            "notes": null
        })
    });
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let mut zip = zip::ZipWriter::new(std::fs::File::create(path).unwrap());
    let stored =
        zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
    let deflated = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    zip.start_file("manifest.json", stored).unwrap();
    zip.write_all(&serde_json::to_vec(&manifest).unwrap())
        .unwrap();
    for (name, bytes) in entries {
        zip.start_file(*name, deflated).unwrap();
        zip.write_all(bytes).unwrap();
    }
    zip.finish().unwrap();
}
