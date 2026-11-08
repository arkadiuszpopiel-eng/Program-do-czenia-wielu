//! Konfiguracja mostów (klucze `[agent_backends]`; zmienia ją tylko użytkownik).

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::Duration;

use agent_backends_contract::{BridgeKind, CliPin, DEFAULT_APPROVAL_TIMEOUT_MS, LaunchPolicy};

/// Program CLI mostu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BridgeProgram {
    /// Ścieżka bezwzględna albo sama nazwa (szukana w PATH).
    pub program: PathBuf,
    /// Przypięcie wersji (i opcjonalnie hasha); pusta lista wersji = trasa wyłączona.
    pub pin: CliPin,
}

/// Konfiguracja backendu.
#[derive(Debug, Clone)]
pub struct BridgeConfig {
    /// Skonfigurowane mosty.
    pub bridges: BTreeMap<BridgeKind, BridgeProgram>,
    /// Polityka uruchamiania (zgody na harmonogram).
    pub launch: LaunchPolicy,
    /// Katalog plików roboczych mostu (konfiguracja MCP z tokenem, 0600) — poza worktree.
    pub runtime_dir: PathBuf,
    /// Limit oczekiwania na decyzję w sprawie uprawnienia (po nim — odmowa).
    pub approval_timeout: Duration,
    /// Limit długości linii strumienia CLI (dłuższe są pomijane z ostrzeżeniem).
    pub max_line_bytes: usize,
    /// Claude Code: `--include-partial-messages` (fragmenty tekstu ≤ 1 s; do potwierdzenia w spike (b)).
    pub claude_partial_messages: bool,
    /// Limit czasu `--version`.
    pub version_timeout: Duration,
    /// Limit czasu na start sesji `codex app-server` (initialize/thread/turn).
    pub codex_handshake_timeout: Duration,
    /// Ile czekać na zakończenie procesu po zabiciu drzewa.
    pub kill_grace: Duration,
}

impl BridgeConfig {
    /// Konfiguracja bez mostów (wszystko trzeba jawnie dodać) z katalogiem roboczym.
    pub fn new(runtime_dir: impl Into<PathBuf>) -> Self {
        Self {
            bridges: BTreeMap::new(),
            launch: LaunchPolicy::default(),
            runtime_dir: runtime_dir.into(),
            approval_timeout: Duration::from_millis(DEFAULT_APPROVAL_TIMEOUT_MS),
            max_line_bytes: crate::lines::DEFAULT_MAX_LINE_BYTES,
            claude_partial_messages: true,
            version_timeout: Duration::from_secs(5),
            codex_handshake_timeout: Duration::from_secs(30),
            kill_grace: Duration::from_millis(1500),
        }
    }

    /// Dodaje most.
    #[must_use]
    pub fn with_bridge(
        mut self,
        kind: BridgeKind,
        program: impl Into<PathBuf>,
        pin: CliPin,
    ) -> Self {
        self.bridges.insert(
            kind,
            BridgeProgram {
                program: program.into(),
                pin,
            },
        );
        self
    }
}
