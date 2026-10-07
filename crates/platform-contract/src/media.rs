//! Priorytet wątku audio (MMCSS) i wolne miejsce na dysku.
//!
//! MMCSS („Pro Audio”): wątek przetwarzania audio dostaje priorytet harmonogramu multimediów
//! (`AvSetMmThreadCharacteristicsW`) — mniej przerw przy obciążeniu UI/LLM (PLAN §6.4). Zwrot
//! priorytetu w `Drop` uchwytu [`ThreadBoost`], który nie jest `Send` (musi wrócić na tym samym
//! wątku, `AvRevertMmThreadCharacteristics`).

use std::fmt;
use std::marker::PhantomData;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::PlatformError;

/// Zadanie MMCSS (`HKLM\…\Multimedia\SystemProfile\Tasks`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MmcssTask {
    /// „Pro Audio” — wątek przetwarzania audio w czasie rzeczywistym.
    ProAudio,
    /// „Audio”.
    Audio,
    /// „Capture”.
    Capture,
}

impl MmcssTask {
    /// Nazwa zadania w rejestrze.
    pub fn name(self) -> &'static str {
        match self {
            Self::ProAudio => "Pro Audio",
            Self::Audio => "Audio",
            Self::Capture => "Capture",
        }
    }
}

/// Podniesiony priorytet bieżącego wątku; `Drop` przywraca poprzedni (RAII). Nie jest `Send`.
pub struct ThreadBoost {
    task: MmcssTask,
    revert: Option<Box<dyn FnOnce()>>,
    _not_send: PhantomData<*const ()>,
}

impl ThreadBoost {
    /// Uchwyt z funkcją przywracającą (wołana dokładnie raz, w `Drop`).
    pub fn new(task: MmcssTask, revert: impl FnOnce() + 'static) -> Self {
        Self {
            task,
            revert: Some(Box::new(revert)),
            _not_send: PhantomData,
        }
    }

    /// Zadanie.
    pub fn task(&self) -> MmcssTask {
        self.task
    }
}

impl fmt::Debug for ThreadBoost {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ThreadBoost")
            .field("task", &self.task)
            .finish_non_exhaustive()
    }
}

impl Drop for ThreadBoost {
    fn drop(&mut self) {
        if let Some(revert) = self.revert.take() {
            revert();
        }
    }
}

/// Port MMCSS.
pub trait MmcssPort: Send + Sync {
    /// Podnosi priorytet bieżącego wątku do zadania `task`.
    fn boost_current_thread(&self, task: MmcssTask) -> Result<ThreadBoost, PlatformError>;
}

/// Miejsce na woluminie.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiskSpace {
    /// Dostępne dla wywołującego (z uwzględnieniem limitów).
    pub available_bytes: u64,
    /// Rozmiar woluminu.
    pub total_bytes: u64,
    /// Wolne łącznie.
    pub free_bytes: u64,
}

/// Port informacji o dysku.
pub trait DiskPort: Send + Sync {
    /// Wolne miejsce na woluminie zawierającym `path` (katalog musi istnieć).
    fn free_disk_space(&self, path: &Path) -> Result<DiskSpace, PlatformError>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    use std::rc::Rc;

    #[test]
    fn boost_reverts_exactly_once() {
        let calls = Rc::new(Cell::new(0));
        let c = calls.clone();
        let boost = ThreadBoost::new(MmcssTask::ProAudio, move || c.set(c.get() + 1));
        assert_eq!(boost.task().name(), "Pro Audio");
        assert!(format!("{boost:?}").contains("ProAudio"));
        drop(boost);
        assert_eq!(calls.get(), 1);
        assert_eq!(MmcssTask::Audio.name(), "Audio");
        assert_eq!(MmcssTask::Capture.name(), "Capture");
    }
}
