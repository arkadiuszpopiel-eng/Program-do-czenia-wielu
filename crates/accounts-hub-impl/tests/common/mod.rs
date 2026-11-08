//! Wspólne budowanie huba w testach (sekrety w pamięci, skryptowany tester, deterministyczne id).

#![allow(dead_code, clippy::unwrap_used, clippy::expect_used)]

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use accounts_hub_contract::ProviderCatalogEntry;
use accounts_hub_fake::{MemorySecretStore, ScriptedConnectionTester, ScriptedModelLister};
use accounts_hub_impl::{AccountsHubService, HubBuilder};
use chrono::{TimeZone, Utc};

/// Budowniczy z portami-atrapami, zegarem 2026-09-30 12:00 UTC i id `acc-1`, `acc-2`…
pub fn builder(catalog: Vec<ProviderCatalogEntry>, secrets: Arc<MemorySecretStore>) -> HubBuilder {
    let counter = Arc::new(AtomicU64::new(0));
    AccountsHubService::builder(
        catalog,
        secrets,
        Arc::new(ScriptedConnectionTester::by_key_prefix()),
        Arc::new(ScriptedModelLister::by_key_prefix()),
    )
    .clock(|| Utc.with_ymd_and_hms(2026, 9, 30, 12, 0, 0).unwrap())
    .id_suffixes(move || (counter.fetch_add(1, Ordering::Relaxed) + 1).to_string())
}

/// Hub bez repozytorium.
pub fn hub(catalog: Vec<ProviderCatalogEntry>) -> AccountsHubService {
    builder(catalog, Arc::new(MemorySecretStore::new()))
        .build()
        .unwrap()
}

/// Katalog `providers-catalog` z repo.
pub fn repo_catalog_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../providers-catalog")
}

/// Unikalny katalog tymczasowy dla testu.
pub fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("alfa-accounts-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}
