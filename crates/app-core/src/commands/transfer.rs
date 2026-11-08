//! Komendy `transfer_*` ⟶ port `TransferPort` (moduł `transfer`; hasła tylko w argumentach).
//! Plik importu UI wskazuje jednorazowym uchwytem (dialog, wynik podglądu, `backups_restore`).

use crate::core::AppCore;
use crate::dto::{
    ExportRequest, ExportResult, ImportRequest, ImportResult, InspectResult, SecretInput,
};
use crate::error::AppError;

impl AppCore {
    /// `transfer_export` ⟶ natywny dialog; sekrety nigdy (eksportu sekretów nie ma — CX-a).
    pub async fn transfer_export(&self, request: ExportRequest) -> Result<ExportResult, AppError> {
        self.inner.transfer.export(request).await
    }

    /// `transfer_inspect` ⟶ dialog otwarcia (gdy `handle = None`) albo jednorazowy uchwyt + dry-run.
    pub async fn transfer_inspect(
        &self,
        password: Option<SecretInput>,
        handle: Option<String>,
    ) -> Result<InspectResult, AppError> {
        self.inner.transfer.inspect(password, handle).await
    }

    /// `backups_restore` („Przywróć…”): kopia z katalogu kopii (nazwa z listy) → jednorazowy
    /// uchwyt dla `transfer_inspect` (ważny 15 s).
    pub async fn backups_restore(&self, file: String) -> Result<String, AppError> {
        let view = self.inner.work.files.backups().view();
        let entry = view
            .entries
            .into_iter()
            .find(|e| e.file == file)
            .ok_or_else(|| AppError::not_found(format!("Brak kopii „{file}” w katalogu kopii.")))?;
        let path = std::path::PathBuf::from(entry.path);
        self.inner.transfer.restore_handle(path)
    }

    /// `transfer_import` (snapshot przed importem).
    pub async fn transfer_import(&self, request: ImportRequest) -> Result<ImportResult, AppError> {
        self.inner.transfer.import(request).await
    }

    /// `transfer_rollback`.
    pub async fn transfer_rollback(&self, snapshot_id: String) -> Result<(), AppError> {
        self.inner.transfer.rollback(&snapshot_id).await
    }
}
