//! Bramka uruchomienia mostu (przed jakimkolwiek procesem zadania). Kolejność:
//! specyfikacja → most skonfigurowany → pochodzenie (wyzwalacz/Ulepszacz nigdy) → trasa
//! w `compliance` (wyłączona = 0 procesów) → program w PATH → `--version` przypięta
//! (lista z konfiguracji ∩ przypięcie z rejestru) → hash pliku (opcjonalnie).

use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{SystemTime, UNIX_EPOCH};

use agent_backends_contract::{
    BackendError, BridgeKind, LaunchOrigin, TaskSpec, check_origin, check_version,
};
use compliance_contract::Compliance;

use crate::config::BridgeConfig;
use crate::process::{locate, probe_version, sha256_file};

/// Maksymalna długość polecenia (znaki).
pub const MAX_PROMPT_CHARS: usize = 200_000;

/// Dopuszczone uruchomienie.
#[derive(Debug, Clone)]
pub struct Admitted {
    /// Program CLI.
    pub program: PathBuf,
    /// Przypięta wersja.
    pub version: String,
}

type VersionKey = (PathBuf, Option<SystemTime>, u64);

/// Bramka.
pub struct Gate {
    compliance: Arc<dyn Compliance>,
    config: Arc<BridgeConfig>,
    scheduled: Mutex<(u64, BTreeMap<BridgeKind, u32>)>,
    versions: Mutex<HashMap<VersionKey, Option<String>>>,
    probes: AtomicU64,
}

fn today() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() / 86_400)
        .unwrap_or(0)
}

fn valid_name(s: &str) -> bool {
    !s.is_empty() && s.len() <= 200 && !s.chars().any(|c| c == ',' || c.is_control())
}

/// Walidacja specyfikacji (bez dostępu do systemu).
pub fn validate(spec: &TaskSpec) -> Result<(), BackendError> {
    let chars = spec.prompt.chars().count();
    if spec.prompt.trim().is_empty() || chars > MAX_PROMPT_CHARS {
        return Err(BackendError::InvalidSpec(format!(
            "polecenie puste albo dłuższe niż {MAX_PROMPT_CHARS} znaków"
        )));
    }
    if !spec
        .allowed_tools
        .iter()
        .chain(&spec.disallowed_tools)
        .all(|t| valid_name(t))
    {
        return Err(BackendError::InvalidSpec(
            "niepoprawna nazwa narzędzia".into(),
        ));
    }
    if spec
        .model
        .as_deref()
        .is_some_and(|m| !valid_name(m) || m.starts_with('-'))
    {
        return Err(BackendError::InvalidSpec("niepoprawna nazwa modelu".into()));
    }
    if spec
        .session
        .as_ref()
        .is_some_and(|s| s.bridge != spec.bridge || !valid_name(&s.id) || s.id.starts_with('-'))
    {
        return Err(BackendError::InvalidSpec(
            "niepoprawna sesja do wznowienia".into(),
        ));
    }
    Ok(())
}

impl Gate {
    /// Nowa bramka.
    pub fn new(compliance: Arc<dyn Compliance>, config: Arc<BridgeConfig>) -> Self {
        Self {
            compliance,
            config,
            scheduled: Mutex::new((today(), BTreeMap::new())),
            versions: Mutex::new(HashMap::new()),
            probes: AtomicU64::new(0),
        }
    }

    /// Liczba procesów `--version` uruchomionych przez bramkę.
    pub fn probes(&self) -> u64 {
        self.probes.load(Ordering::SeqCst)
    }

    fn scheduled(&self) -> MutexGuard<'_, (u64, BTreeMap<BridgeKind, u32>)> {
        let mut g = self.scheduled.lock().unwrap_or_else(|p| p.into_inner());
        let day = today();
        if g.0 != day {
            *g = (day, BTreeMap::new());
        }
        g
    }

    async fn version(&self, program: &PathBuf) -> Option<String> {
        let meta = std::fs::metadata(program).ok();
        let key = (
            program.clone(),
            meta.as_ref().and_then(|m| m.modified().ok()),
            meta.map_or(0, |m| m.len()),
        );
        if let Some(v) = self
            .versions
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .get(&key)
        {
            return v.clone();
        }
        self.probes.fetch_add(1, Ordering::SeqCst);
        let found = probe_version(program, self.config.version_timeout).await;
        self.versions
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .insert(key, found.clone());
        found
    }

    /// Sprawdza wszystkie warunki; przy sukcesie zalicza uruchomienie z harmonogramu.
    pub async fn check(&self, spec: &TaskSpec) -> Result<Admitted, BackendError> {
        validate(spec)?;
        let bridge = self
            .config
            .bridges
            .get(&spec.bridge)
            .ok_or_else(|| BackendError::BridgeUnavailable(spec.bridge.to_string()))?;
        let today_count = self.scheduled().1.get(&spec.bridge).copied().unwrap_or(0);
        check_origin(&spec.origin, spec.bridge, &self.config.launch, today_count)
            .map_err(|refusal| BackendError::LaunchRefused { refusal })?;
        let route = spec
            .bridge
            .route_id()
            .ok_or_else(|| BackendError::BridgeUnavailable(spec.bridge.to_string()))?;
        let decision = self.compliance.route_allowed(&route, spec.privacy);
        if !decision.allowed {
            return Err(BackendError::RouteNotAllowed {
                route: route.to_string(),
                reason: decision.reason.to_string(),
            });
        }
        let program = locate(&bridge.program).ok_or_else(|| BackendError::CliNotFound {
            program: bridge.program.to_string_lossy().into_owned(),
        })?;
        let found = self.version(&program).await;
        let registry_pin = self
            .compliance
            .route(&route)
            .and_then(|r| r.cli_pinned_version);
        let version = check_version(
            spec.bridge,
            found.as_deref(),
            &bridge.pin,
            registry_pin.as_deref(),
        )?;
        if let Some(expected) = &bridge.pin.sha256
            && !sha256_file(&program).await?.eq_ignore_ascii_case(expected)
        {
            return Err(BackendError::BinaryHashMismatch {
                program: spec.bridge.program().to_owned(),
            });
        }
        if matches!(spec.origin, LaunchOrigin::Scheduled { .. }) {
            *self.scheduled().1.entry(spec.bridge).or_insert(0) += 1;
        }
        Ok(Admitted { program, version })
    }
}
