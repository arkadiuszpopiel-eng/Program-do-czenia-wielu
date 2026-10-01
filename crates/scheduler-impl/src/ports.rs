//! Porty otoczenia: budżet tła (`cost-meter`) i magazyn stanu w pliku (restart = wznowienie).

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use chrono::NaiveDate;
use cost_meter_contract::{BudgetDecision, CostMeter, Month, Spent, TotalsQuery, evaluate};
use scheduler_contract::{Snapshot, SnapshotStore};

/// Decyzja budżetu tła dla zadania klasy `Background` o szacowanym koszcie (mikro-PLN).
pub trait BackgroundBudget: Send + Sync {
    /// Allow / Warn / Block.
    fn check(&self, estimate_micro_pln: u64) -> BudgetDecision;
}

/// Bez limitu (testy, zanim `cost-meter` zostanie podpięty).
#[derive(Debug, Default, Clone, Copy)]
pub struct UnlimitedBudget;

impl BackgroundBudget for UnlimitedBudget {
    fn check(&self, _estimate_micro_pln: u64) -> BudgetDecision {
        BudgetDecision::Allow
    }
}

/// Budżet tła z `cost-meter`: ta sama czysta reguła co `CostMeter::check_budget` (`evaluate`),
/// liczona synchronicznie z bieżącej konfiguracji i sum miesiąca (bez blokowania sterownika).
pub struct CostMeterBudget {
    meter: Arc<dyn CostMeter>,
    today: fn() -> NaiveDate,
}

fn local_today() -> NaiveDate {
    chrono::Local::now().date_naive()
}

impl CostMeterBudget {
    /// Adapter z dniem lokalnym z zegara systemu.
    pub fn new(meter: Arc<dyn CostMeter>) -> Self {
        Self {
            meter,
            today: local_today,
        }
    }

    /// Adapter ze stałym dniem (testy).
    pub fn with_today(meter: Arc<dyn CostMeter>, today: fn() -> NaiveDate) -> Self {
        Self { meter, today }
    }
}

impl BackgroundBudget for CostMeterBudget {
    fn check(&self, estimate_micro_pln: u64) -> BudgetDecision {
        let month = Month::of((self.today)());
        let spent = Spent {
            month_micro_pln: self.meter.totals(&TotalsQuery::Month { month }).micro_pln,
            background_micro_pln: self
                .meter
                .totals(&TotalsQuery::Background { month })
                .micro_pln,
            provider_micro_pln: 0,
        };
        evaluate(&self.meter.budget(), spent, estimate_micro_pln, true, None)
    }
}

/// Stan schedulera w pliku JSON (`%LOCALAPPDATA%\Alfa\scheduler\state.json`): zapis atomowy
/// (plik tymczasowy + zmiana nazwy). Ładunki zadań mogą zawierać cele użytkownika — w kompozycji
/// aplikacji katalog leży w profilu użytkownika (ACL), docelowo magazyn szyfrowany (`lib-sqlstore`).
/// Zapisy są szeregowane, a stan starszy (mniejsza `revision`) nie nadpisuje nowszego.
#[derive(Debug)]
pub struct FileSnapshotStore {
    path: PathBuf,
    last_revision: Mutex<Option<u64>>,
}

impl FileSnapshotStore {
    /// Magazyn w pliku `path`.
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            last_revision: Mutex::new(None),
        }
    }

    /// Ścieżka pliku.
    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl SnapshotStore for FileSnapshotStore {
    fn load(&self) -> Result<Option<Snapshot>, String> {
        match std::fs::read(&self.path) {
            Ok(bytes) => serde_json::from_slice(&bytes)
                .map(Some)
                .map_err(|e| format!("uszkodzony stan schedulera: {e}")),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(format!("odczyt stanu schedulera: {e}")),
        }
    }

    fn save(&self, snapshot: &Snapshot) -> Result<(), String> {
        let mut last = self.last_revision.lock().unwrap_or_else(|p| p.into_inner());
        if last.is_some_and(|r| snapshot.revision < r) {
            return Ok(());
        }
        if let Some(dir) = self.path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| format!("katalog stanu: {e}"))?;
        }
        let bytes = serde_json::to_vec(snapshot).map_err(|e| e.to_string())?;
        let tmp = self.path.with_extension("json.tmp");
        std::fs::write(&tmp, bytes).map_err(|e| format!("zapis stanu: {e}"))?;
        std::fs::rename(&tmp, &self.path).map_err(|e| format!("zapis stanu: {e}"))?;
        *last = Some(snapshot.revision);
        Ok(())
    }
}
