//! Broker w procesie aplikacji (tryb deweloperski, Linux/CI, brak binarek Jądra obok aplikacji):
//! polityka bazowa profilu właściciela, Audyt w pliku z łańcuchem SHA-256 i kotwicą w katalogu
//! `broker-dev`, ten sam port procesów co narzędzia agentek (kill-switch zabija ich drzewa).
//! Bez Broker-UI każda prośba o zgodę jest odrzucana (`NoApprovalWindow`).

use std::path::{Path, PathBuf};
use std::sync::Arc;

use compliance_contract::PathEnv;
use core_bus_contract::EventBus;
use platform_contract::ProcessPort;
use risk_classifier_contract::RiskClassifier;
use safety_broker_contract::KernelPolicy;
use safety_broker_impl::audit::{BrokerAuditWriter, FileAnchorStore};
use safety_broker_impl::{BrokerConfig, BrokerEngine, KeyMode};

use crate::KernelBroker;

/// Środowisko ścieżek Brokera: profil Windows (`USERPROFILE`); poza Windows — profil syntetyczny
/// (tryb deweloperski: decyzje i Audyt bez dostępu do plików systemu).
pub fn path_env() -> (String, PathEnv) {
    let profile = std::env::var("USERPROFILE")
        .ok()
        .filter(|p| cfg!(windows) && !p.is_empty())
        .unwrap_or_else(|| r"C:\Users\alfa".to_owned());
    let env = PathEnv::windows_profile(&profile);
    (profile, env)
}

/// Środowisko ścieżek dla katalogów aplikacji: profil właściciela = katalog nadrzędny
/// `user_root` (`%USERPROFILE%\Alfa` → `%USERPROFILE%`; w testach — katalog tymczasowy), żeby
/// zakresy Brokera i narzędzi agentek dotyczyły tych samych ścieżek, na których działa `FsPort`.
pub fn path_env_for(user_root: &Path) -> (String, PathEnv) {
    match user_root.parent().and_then(|p| p.to_str()) {
        Some(profile) if !profile.is_empty() => {
            (profile.to_owned(), PathEnv::windows_profile(profile))
        }
        _ => path_env(),
    }
}

/// Katalog danych Brokera w trybie deweloperskim.
pub fn dev_dir(local: &Path) -> PathBuf {
    local.join("broker-dev")
}

/// Zależności Brokera w procesie.
pub struct InprocDeps<'a> {
    /// Katalog użytkownika Alfy (`%USERPROFILE%\Alfa`) — profil właściciela to jego rodzic.
    pub user_root: &'a Path,
    /// Katalog danych lokalnych (`%LOCALAPPDATA%\Alfa`) — Audyt w `broker-dev`.
    pub local: &'a Path,
    /// Port procesów narzędzi (ta sama tablica uchwytów Job Objects co `shell_run`).
    pub processes: Arc<dyn ProcessPort>,
    /// Magistrala (cisza audio przy kill-switchu).
    pub bus: Arc<dyn EventBus>,
    /// Klasyfikator ryzyka (`None` = tabela z kontraktu z progami z polityki Jądra).
    pub classifier: Option<Arc<dyn RiskClassifier>>,
}

/// Silnik Brokera w procesie.
pub fn engine(d: InprocDeps<'_>) -> Result<Arc<BrokerEngine>, String> {
    let (profile, env) = path_env_for(d.user_root);
    let dir = dev_dir(d.local);
    let policy = KernelPolicy::baseline(&profile, &dir.to_string_lossy())
        .or_else(|_| KernelPolicy::baseline(&profile, r"C:\ProgramData\AlfaBroker"))
        .map_err(|e| e.to_string())?;
    let clock = Arc::new(watchdog_contract::SystemClock);
    let audit = BrokerAuditWriter::open(
        dir.join("audit.ndjson"),
        Arc::new(FileAnchorStore::new(dir.join("anchor.json"))),
        Arc::new(core_log_contract::RegexRedactor::default()),
        clock.clone(),
        "broker-dev",
        None,
    )
    .map_err(|e| format!("Audyt: {e}"))?;
    let config = BrokerConfig {
        policy,
        env,
        key_mode: KeyMode::Random,
    };
    let mut engine = BrokerEngine::new(config, clock, Arc::new(audit), d.processes)
        .map_err(|e| e.to_string())?
        .with_bus(d.bus);
    if let Some(c) = d.classifier {
        engine = engine.with_classifier(c);
    }
    Ok(Arc::new(engine))
}

/// Broker w procesie jako [`KernelBroker`].
pub fn kernel(d: InprocDeps<'_>) -> Result<KernelBroker, String> {
    engine(d).map(KernelBroker::in_process)
}
