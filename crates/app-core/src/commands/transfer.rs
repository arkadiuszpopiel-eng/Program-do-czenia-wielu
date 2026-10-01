//! Komendy `transfer_*` ⟶ port `TransferPort` (moduł `transfer`; hasła tylko w argumentach).

use crate::core::AppCore;
use crate::dto::{
    ExportRequest, ExportResult, ImportRequest, ImportResult, InspectResult, SecretInput,
};
use crate::error::AppError;

impl AppCore {
    /// `transfer_export` ⟶ natywny dialog; sekrety nigdy.
    pub async fn transfer_export(&self, request: ExportRequest) -> Result<ExportResult, AppError> {
        self.inner.transfer.export(request).await
    }

    /// `transfer_export_secrets` ⟶ natywny dialog; osobna, jawna paczka sekretów — zawsze
    /// szyfrowana hasłem (min. 8 znaków), nigdy w zwykłym eksporcie.
    pub async fn transfer_export_secrets(
        &self,
        password: SecretInput,
    ) -> Result<ExportResult, AppError> {
        self.inner.transfer.export_secrets(password).await
    }

    /// `transfer_inspect` ⟶ dialog otwarcia (gdy `path = None`) + dry-run.
    pub async fn transfer_inspect(
        &self,
        password: Option<SecretInput>,
        path: Option<String>,
    ) -> Result<InspectResult, AppError> {
        self.inner.transfer.inspect(password, path).await
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
