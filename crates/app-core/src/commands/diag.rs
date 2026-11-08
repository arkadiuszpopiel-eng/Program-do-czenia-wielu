//! Komendy diagnostyki komputera (`diag_*`, Ustawienia → Diagnostyka komputera) — delegują do
//! `app-diag::DiagApp`. Tylko pomiar: nic nie jest naprawiane ani zmieniane w systemie.

use crate::core::AppCore;
use crate::dto::{CheckRun, DiagView};
use crate::error::AppError;

impl AppCore {
    /// `diag_state`: ostatni zapis z oceną, ostatnie testy, dostępne testy, postęp.
    pub async fn diag_state(&self) -> Result<DiagView, AppError> {
        Ok(self.inner.diag.state())
    }

    /// `diag_snapshot`: nowy zapis parametrów (kilka sekund).
    pub async fn diag_snapshot(&self) -> Result<DiagView, AppError> {
        self.inner.diag.snapshot().await
    }

    /// `diag_run`: testy z kluczy (`cpu_single`, `disk:C:`…); postęp w `diag_state`.
    pub async fn diag_run(&self, tests: Vec<String>) -> Result<CheckRun, AppError> {
        self.inner.diag.run(tests).await
    }

    /// `diag_cancel`: przerywa trwające testy.
    pub async fn diag_cancel(&self) -> Result<(), AppError> {
        self.inner.diag.cancel();
        Ok(())
    }

    /// `diag_runs`: ostatnie przebiegi testów (najnowsze pierwsze).
    pub async fn diag_runs(&self, limit: u32) -> Result<Vec<CheckRun>, AppError> {
        Ok(self.inner.diag.runs(limit))
    }

    /// `diag_export`: raport Markdown w folderze `Diagnostyka`; zwraca ścieżkę pliku.
    pub async fn diag_export(&self) -> Result<String, AppError> {
        self.inner.diag.export().await
    }
}
