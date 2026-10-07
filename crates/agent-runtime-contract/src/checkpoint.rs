//! Checkpointy przebiegu: pełny stan pętli w punkcie atomowym (historia append-only, kroki,
//! zużycie, taint, proweniencja, wywołania w toku) — wznowienie po restarcie bez ponownego
//! wykonania przerwanych akcji.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use core_bus_contract::RunId;
use providers_contract::{Message, ToolUse};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use safety_broker_contract::TaintSource;

use crate::event::{RunOutcome, UsageTotals};
use crate::options::RunOptions;
use crate::spec::RunSpec;

/// Wersja formatu checkpointu.
pub const CHECKPOINT_VERSION: u32 = 1;

/// Stan pętli w punkcie atomowym.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Checkpoint {
    /// Wersja formatu.
    pub version: u32,
    /// Przebieg.
    pub run: RunId,
    /// Numer checkpointu (rośnie).
    pub seq: u64,
    /// Specyfikacja.
    pub spec: RunSpec,
    /// Historia przebiegu (append-only; bez promptu systemowego).
    pub messages: Vec<Message>,
    /// Zużycie (kroki, tokeny, koszt, czas).
    pub usage: UsageTotals,
    /// Sesja widziała niezaufaną treść.
    pub tainted: bool,
    /// Treść zaufana (cel, wiadomości właściciela) — małymi literami, z limitem.
    pub trusted_text: String,
    /// Treść niezaufana (wyniki narzędzi) — małymi literami, z limitem.
    pub untrusted_text: String,
    /// Wywołania narzędzi zlecone, ale bez zapisanego wyniku (restart w trakcie).
    pub pending: Vec<ToolUse>,
    /// Odciski ostatnich wywołań (detektor pętli).
    pub recent_calls: Vec<String>,
    /// Faza weryfikacji.
    pub verifying: bool,
    /// Rundy weryfikacji.
    pub verify_rounds: u32,
    /// Plan już ogłoszony.
    pub planned: bool,
    /// Pauza.
    pub paused: bool,
    /// Wynik (przebieg zakończony).
    pub finished: Option<RunOutcome>,
    /// Opcje v1 (delegacja, Krytyczka, koperta uprawnień, pochodzenie); domyślne = v0.
    #[serde(default)]
    pub options: RunOptions,
    /// Pierwsze źródło niezaufanej treści (taint dziedziczony przez podprzebiegi).
    #[serde(default)]
    pub taint_source: Option<TaintSource>,
    /// Zużycie podprzebiegów (delegacje, Krytyczka) — liczone do budżetu tego przebiegu.
    #[serde(default)]
    pub delegated: UsageTotals,
}

impl Checkpoint {
    /// Stan początkowy dla specyfikacji.
    pub fn initial(run: RunId, spec: RunSpec) -> Self {
        Self::with_options(run, spec, RunOptions::default())
    }

    /// Stan początkowy z opcjami v1 (taint i proweniencja odziedziczone po rodzicu).
    pub fn with_options(run: RunId, spec: RunSpec, options: RunOptions) -> Self {
        let taint_source = options.inherited_taint.clone();
        let trusted_text = options.trusted_context.clone();
        let untrusted_text = options.untrusted_context.clone();
        Self {
            version: CHECKPOINT_VERSION,
            run,
            seq: 0,
            spec,
            messages: Vec::new(),
            usage: UsageTotals::default(),
            tainted: taint_source.is_some(),
            trusted_text,
            untrusted_text,
            pending: Vec::new(),
            recent_calls: Vec::new(),
            verifying: false,
            verify_rounds: 0,
            planned: false,
            paused: false,
            finished: None,
            options,
            taint_source,
            delegated: UsageTotals::default(),
        }
    }
}

/// Błąd magazynu checkpointów.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("magazyn checkpointów: {0}")]
pub struct CheckpointError(pub String);

/// Magazyn checkpointów (w sesji: katalog przebiegów; w testach: pamięć).
pub trait CheckpointStore: Send + Sync {
    /// Zapisuje (zastępuje poprzedni) checkpoint przebiegu — atomowo.
    fn save(&self, checkpoint: &Checkpoint) -> Result<(), CheckpointError>;
    /// Ostatni checkpoint przebiegu.
    fn latest(&self, run: &RunId) -> Result<Option<Checkpoint>, CheckpointError>;
    /// Przebiegi z checkpointem (rosnąco).
    fn runs(&self) -> Result<Vec<RunId>, CheckpointError>;
}

/// Magazyn w pamięci (współdzielony między „restartami” w testach przez `Clone`).
#[derive(Debug, Clone, Default)]
pub struct MemCheckpointStore {
    inner: Arc<Mutex<BTreeMap<RunId, Checkpoint>>>,
}

impl MemCheckpointStore {
    fn lock(&self) -> std::sync::MutexGuard<'_, BTreeMap<RunId, Checkpoint>> {
        self.inner.lock().unwrap_or_else(|p| p.into_inner())
    }
}

impl CheckpointStore for MemCheckpointStore {
    fn save(&self, checkpoint: &Checkpoint) -> Result<(), CheckpointError> {
        self.lock()
            .insert(checkpoint.run.clone(), checkpoint.clone());
        Ok(())
    }

    fn latest(&self, run: &RunId) -> Result<Option<Checkpoint>, CheckpointError> {
        Ok(self.lock().get(run).cloned())
    }

    fn runs(&self) -> Result<Vec<RunId>, CheckpointError> {
        Ok(self.lock().keys().cloned().collect())
    }
}
