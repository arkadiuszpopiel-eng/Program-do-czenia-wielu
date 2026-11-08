//! Stan współdzielony przez moduł i narzędzia: silnik, bramka Brokera, port hosta, magazyn,
//! biblioteka, pamięć podręczna skompilowanych modułów (klucz: SHA-256), magistrala.
//! Moduł trafia do pamięci podręcznej wyłącznie po weryfikacji hasha bajtów z magazynu
//! i integralności zatwierdzenia rekordu.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, MutexGuard, RwLock};

use core_bus_contract::{Event, EventBus, Level};
use plugin_runtime_contract::{
    LoadError, PluginError, PluginHost, PluginLibrary, PluginManifest, PluginRecord, PluginState,
    PluginStore, check_approved, event_kind, events, sha256_hex,
};
use tools_common_contract::BrokerGate;
use wasmtime::component::InstancePre;

use crate::engine::SandboxEngine;
use crate::sandbox::StoreData;
use crate::{RuntimeConfig, lock};

/// Stan współdzielony przez moduł i narzędzia.
pub(crate) struct Inner {
    pub(crate) engine: SandboxEngine,
    pub(crate) gate: BrokerGate,
    pub(crate) host: Arc<dyn PluginHost>,
    pub(crate) store: Arc<dyn PluginStore>,
    pub(crate) lib: Mutex<PluginLibrary>,
    pub(crate) cache: Mutex<BTreeMap<String, InstancePre<StoreData>>>,
    pub(crate) bus: RwLock<Option<Arc<dyn EventBus>>>,
    pub(crate) permits: tokio::sync::Semaphore,
    pub(crate) config: RuntimeConfig,
    pub(crate) clock: fn() -> u64,
}

impl Inner {
    pub(crate) fn lib(&self) -> MutexGuard<'_, PluginLibrary> {
        lock(&self.lib)
    }

    pub(crate) async fn publish(&self, events: Vec<Event>) {
        let bus = self.bus.read().unwrap_or_else(|p| p.into_inner()).clone();
        if let Some(bus) = bus {
            for e in events {
                let _ = bus.publish(e).await;
            }
        }
    }

    /// Kompilacja na wątku blokującym (kod z zewnątrz — nie na wątku runtime).
    pub(crate) async fn compile(
        self: &Arc<Self>,
        bytes: Vec<u8>,
    ) -> Result<InstancePre<StoreData>, LoadError> {
        let me = self.clone();
        tokio::task::spawn_blocking(move || me.engine.compile(&bytes))
            .await
            .unwrap_or_else(|e| Err(LoadError::Invalid(e.to_string())))
    }

    /// Moduł gotowy do uruchomienia: z pamięci albo z magazynu po weryfikacji SHA-256.
    pub(crate) async fn prepared(
        self: &Arc<Self>,
        record: &PluginRecord,
    ) -> Result<InstancePre<StoreData>, LoadError> {
        let sha = &record.manifest.wasm_sha256;
        if let Some(pre) = lock(&self.cache).get(sha).cloned() {
            return Ok(pre);
        }
        let result = match record.state {
            PluginState::Installed => check_approved(record),
            _ => Ok(()),
        };
        let result = match result {
            Ok(()) => self.load_verified(&record.manifest).await,
            Err(e) => Err(e),
        };
        match &result {
            Ok(pre) => {
                lock(&self.cache).insert(sha.clone(), pre.clone());
            }
            Err(e) => {
                let ev = Event::new(
                    event_kind(events::LOAD_FAILED),
                    Level::Warn,
                    serde_json::json!({
                        "plugin": record.manifest.id,
                        "version": record.manifest.version.to_string(),
                        "wasm_sha256": sha,
                        "error": e,
                    }),
                );
                self.publish(vec![ev]).await;
            }
        }
        result
    }

    async fn load_verified(
        self: &Arc<Self>,
        m: &PluginManifest,
    ) -> Result<InstancePre<StoreData>, LoadError> {
        let bytes = self
            .store
            .get_wasm(&m.wasm_sha256)
            .map_err(LoadError::Invalid)?
            .ok_or(LoadError::MissingModule)?;
        let actual = sha256_hex(&bytes);
        if actual != m.wasm_sha256 {
            return Err(LoadError::HashMismatch {
                expected: m.wasm_sha256.clone(),
                actual,
            });
        }
        self.compile(bytes).await
    }

    /// Zmiana stanu: na kopii, zapis do magazynu, dopiero potem podmiana (błąd zapisu = brak zmiany).
    pub(crate) async fn change<T>(
        &self,
        f: impl FnOnce(&mut PluginLibrary, u64) -> Result<(T, Vec<Event>), PluginError>,
    ) -> Result<T, PluginError> {
        let events = {
            let now = (self.clock)();
            let mut lib = self.lib();
            let mut next = lib.clone();
            let (value, events) = f(&mut next, now)?;
            if !events.is_empty() {
                self.store
                    .save_records(next.records())
                    .map_err(PluginError::Store)?;
            }
            *lib = next;
            (value, events)
        };
        let (value, events) = events;
        self.publish(events).await;
        Ok(value)
    }

    pub(crate) fn evict(&self, records: &[PluginRecord]) {
        let mut cache = lock(&self.cache);
        for r in records {
            cache.remove(&r.manifest.wasm_sha256);
        }
    }
}
