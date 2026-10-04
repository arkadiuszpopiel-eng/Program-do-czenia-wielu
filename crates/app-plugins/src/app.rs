//! Wtyczki w aplikacji: `PluginRuntime` budowany leniwie (silnik wasmtime z wątkiem epok startuje
//! dopiero przy pierwszej komendzie „Wtyczki” albo przy pierwszym rejestrze narzędzi, gdy magazyn
//! ma już rekordy), komendy `plugins_*` wołane wyłącznie z okna właściciela (zatwierdzenie =
//! kliknięcie z hashem przejrzanej wersji, kanał `Ui`), narzędzia aktywnych wtyczek do rejestru
//! agentek — liczone przy każdym odczycie, więc zawsze aktualne po zmianie stanu.

use std::path::PathBuf;
use std::sync::{Arc, OnceLock};

use app_api::AppError;
use app_api::dto::{PluginInfo, PluginInspection, PluginsView};
use base64::Engine as _;
use compliance_contract::{DenyLists, PathEnv};
use core_bus_contract::EventBus;
use core_registry_contract::HealthStatus;
use platform_contract::FsPort;
use plugin_runtime_contract::{
    MAX_WASM_BYTES, PluginApproval, PluginId, PluginManifest, PluginRecord, PluginSource,
    PluginState, Plugins, sha256_hex,
};
use plugin_runtime_impl::{DirPluginStore, PluginDeps, PluginRuntime, RuntimeConfig};
use safety_broker_contract::Broker;
use semver::Version;
use tools_common_contract::Tool;
use undo_journal_contract::UndoJournal;

use crate::host::{AlfaPluginHost, HostDeps};
use crate::net::{EgressClient, HttpsGet};
use crate::problems::{ProblemBus, Problems};
use crate::view;

/// Plik rekordów magazynu (jego obecność = są wtyczki do wczytania przy starcie).
const RECORDS_FILE: &str = "plugins.json";

/// Zależności wtyczek (te same co narzędzi agentek).
#[derive(Clone)]
pub struct PluginsDeps {
    /// Broker (zwykle rejestr kart nad Brokerem) — tokeny operacji hosta za agentkę wywołującą.
    pub broker: Arc<dyn Broker>,
    /// Odczyt plików hosta.
    pub fs: Arc<dyn FsPort>,
    /// Zapis plików hosta (cofanie).
    pub journal: Arc<dyn UndoJournal>,
    /// Środowisko ścieżek.
    pub env: PathEnv,
    /// Deny-listy Jądra.
    pub deny: DenyLists,
    /// Magistrala (`plugin.*`).
    pub bus: Option<Arc<dyn EventBus>>,
    /// Katalog magazynu (`%LOCALAPPDATA%\Alfa\plugins`).
    pub dir: PathBuf,
    /// Klient sieci (`None` — [`EgressClient`]).
    pub net: Option<Arc<dyn HttpsGet>>,
}

struct Ready {
    deps: PluginsDeps,
    pending: bool,
    runtime: OnceLock<Result<Arc<PluginRuntime>, String>>,
}

/// Wtyczki: komendy strony „Wtyczki”, narzędzia dla agentek, zdrowie dla Diagnosty.
pub struct PluginsApp {
    ready: Result<Ready, String>,
    problems: Arc<Problems>,
}

impl std::fmt::Debug for PluginsApp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PluginsApp")
            .field("available", &self.ready.is_ok())
            .finish_non_exhaustive()
    }
}

fn decode(wasm_b64: &str) -> Result<Vec<u8>, AppError> {
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(wasm_b64.trim())
        .map_err(|_| AppError::invalid("Wtyczki: moduł nie jest poprawnym base64."))?;
    if bytes.len() > MAX_WASM_BYTES {
        return Err(AppError::invalid(format!(
            "Wtyczki: moduł większy niż {} MiB.",
            MAX_WASM_BYTES / (1024 * 1024)
        )));
    }
    Ok(bytes)
}

fn version(v: &str) -> Result<Version, AppError> {
    Version::parse(v.trim()).map_err(|e| AppError::invalid(format!("Wtyczki: wersja „{v}”: {e}")))
}

impl PluginsApp {
    /// Wtyczki nad zależnościami (runtime powstaje przy pierwszym użyciu).
    pub fn new(deps: PluginsDeps) -> Self {
        let pending = deps.dir.join(RECORDS_FILE).is_file();
        Self {
            ready: Ok(Ready {
                deps,
                pending,
                runtime: OnceLock::new(),
            }),
            problems: Arc::new(Problems::default()),
        }
    }

    /// Wtyczki niedostępne (np. bez Brokera albo dziennika cofania) — komendy zwracają powód.
    pub fn unavailable(reason: impl Into<String>) -> Self {
        Self {
            ready: Err(reason.into()),
            problems: Arc::new(Problems::default()),
        }
    }

    fn build(&self, r: &Ready) -> Result<Arc<PluginRuntime>, String> {
        let d = &r.deps;
        let store = DirPluginStore::open(&d.dir)?;
        let net = match &d.net {
            Some(n) => n.clone(),
            None => Arc::new(EgressClient::new()?) as Arc<dyn HttpsGet>,
        };
        let host = AlfaPluginHost::new(HostDeps {
            broker: d.broker.clone(),
            fs: d.fs.clone(),
            journal: d.journal.clone(),
            env: d.env.clone(),
            deny: d.deny.clone(),
            net,
        });
        let bus = ProblemBus::new(d.bus.clone(), self.problems.clone());
        PluginRuntime::new(PluginDeps {
            broker: d.broker.clone(),
            host: Arc::new(host),
            store: Arc::new(store),
            bus: Some(Arc::new(bus)),
            config: RuntimeConfig::default(),
        })
        .map(Arc::new)
    }

    /// Runtime (budowany przy pierwszym użyciu; błąd budowy zostaje zapamiętany).
    pub fn runtime(&self) -> Result<Arc<PluginRuntime>, AppError> {
        let r = self.ready.as_ref().map_err(|why| {
            AppError::new(
                app_api::ErrorCode::Unavailable,
                format!("Wtyczki niedostępne: {why}."),
            )
        })?;
        r.runtime
            .get_or_init(|| self.build(r))
            .clone()
            .map_err(|e| AppError::storage(format!("wtyczki: {e}")))
    }

    /// Narzędzia aktywnych wtyczek (bez budowy silnika, gdy wtyczek jeszcze nie ma).
    pub fn tools(&self) -> Vec<Arc<dyn Tool>> {
        let Ok(r) = &self.ready else {
            return Vec::new();
        };
        if r.runtime.get().is_none() && !r.pending {
            return Vec::new();
        }
        self.runtime()
            .map(|rt| Plugins::tools(&*rt))
            .unwrap_or_default()
    }

    /// Zdrowie modułu `plugin-runtime` (świeży problem → `Degraded`).
    pub fn health(&self) -> HealthStatus {
        match &self.ready {
            Err(why) => HealthStatus::Unhealthy(why.clone()),
            Ok(r) => match r.runtime.get() {
                Some(Err(e)) => HealthStatus::Unhealthy(e.clone()),
                _ => self.problems.health(chrono::Utc::now()),
            },
        }
    }

    fn card(rt: &PluginRuntime, r: &PluginRecord) -> PluginInfo {
        let r2 = (r.state == PluginState::Proposed)
            .then(|| rt.r2_proposal(&r.manifest.id, &r.manifest.version).ok())
            .flatten()
            .map(|c| view::r2(&c));
        view::info(r, r2)
    }

    /// `plugins_list`: biblioteka (propozycje na górze), problemy, dostępność.
    pub fn list(&self) -> Result<PluginsView, AppError> {
        let problems = self.problems.list();
        let rt = match self.runtime() {
            Ok(rt) => rt,
            Err(e) => {
                return Ok(PluginsView {
                    available: false,
                    unavailable_reason: Some(e.message),
                    plugins: Vec::new(),
                    problems,
                });
            }
        };
        let mut records = rt.list();
        records.sort_by_key(|r| {
            (
                r.state != PluginState::Proposed,
                r.manifest.id.clone(),
                std::cmp::Reverse(r.manifest.version.clone()),
            )
        });
        Ok(PluginsView {
            available: true,
            unavailable_reason: None,
            plugins: records.iter().map(|r| Self::card(&rt, r)).collect(),
            problems,
        })
    }

    /// `plugins_inspect`: kontrola modułu (nagłówek, importy, eksporty) bez instalacji.
    pub async fn inspect(&self, wasm_b64: &str) -> Result<PluginInspection, AppError> {
        let rt = self.runtime()?;
        let bytes = decode(wasm_b64)?;
        let (sha, len) = (sha256_hex(&bytes), bytes.len() as u64);
        let error = rt.inspect(bytes).await.err();
        Ok(PluginInspection {
            ok: error.is_none(),
            wasm_sha256: sha,
            bytes: len,
            error: error.map(|e| e.to_string()),
        })
    }

    /// `plugins_propose`: manifest + moduł → propozycja (czeka na zatwierdzenie w oknie).
    pub async fn propose(
        &self,
        manifest: serde_json::Value,
        wasm_b64: &str,
    ) -> Result<PluginInfo, AppError> {
        let rt = self.runtime()?;
        let manifest: PluginManifest = serde_json::from_value(manifest)
            .map_err(|e| AppError::invalid(format!("Wtyczki: manifest niepoprawny: {e}")))?;
        let wasm = decode(wasm_b64)?;
        let r = rt
            .propose(manifest, wasm, PluginSource::User)
            .await
            .map_err(view::error)?;
        Ok(Self::card(&rt, &r))
    }

    /// `plugins_approve`: kliknięcie w oknie z hashem przejrzanej wersji → instalacja.
    pub async fn approve(
        &self,
        id: &str,
        version_text: &str,
        reviewed_hash: &str,
    ) -> Result<PluginInfo, AppError> {
        let rt = self.runtime()?;
        let r = rt
            .approve(
                &PluginId::new(id),
                &version(version_text)?,
                PluginApproval::ui(reviewed_hash),
            )
            .await
            .map_err(view::error)?;
        Ok(Self::card(&rt, &r))
    }

    /// `plugins_reject`: odrzucenie propozycji.
    pub async fn reject(&self, id: &str, version_text: &str) -> Result<PluginInfo, AppError> {
        let rt = self.runtime()?;
        let r = rt
            .reject(&PluginId::new(id), &version(version_text)?)
            .await
            .map_err(view::error)?;
        Ok(Self::card(&rt, &r))
    }

    /// `plugins_disable`: narzędzia znikają z rejestru agentek.
    pub async fn disable(&self, id: &str) -> Result<PluginInfo, AppError> {
        let rt = self.runtime()?;
        let r = rt.disable(&PluginId::new(id)).await.map_err(view::error)?;
        Ok(Self::card(&rt, &r))
    }

    /// `plugins_enable`: ponowne włączenie = ponowne zatwierdzenie hashem w oknie.
    pub async fn enable(&self, id: &str, reviewed_hash: &str) -> Result<PluginInfo, AppError> {
        let rt = self.runtime()?;
        let r = rt
            .enable(&PluginId::new(id), PluginApproval::ui(reviewed_hash))
            .await
            .map_err(view::error)?;
        Ok(Self::card(&rt, &r))
    }

    /// `plugins_remove`: usunięcie wszystkich wersji i modułów.
    pub async fn remove(&self, id: &str) -> Result<PluginsView, AppError> {
        self.runtime()?
            .remove(&PluginId::new(id))
            .await
            .map_err(view::error)?;
        self.list()
    }
}
