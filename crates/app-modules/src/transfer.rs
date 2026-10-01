//! `TransferPort` na module `transfer` (`ZipTransfer`): natywne dialogi powłoki, zakres z DTO,
//! podgląd (dry-run) z różnicami, tryby dodaj/scal/zastąp, snapshot przed importem i rollback.
//! Sekrety nigdy w zwykłym eksporcie; osobny eksport sekretów wyłącznie z hasłem. Sesje prywatne
//! nie wchodzą do zwykłego eksportu (moduł pomija je z ostrzeżeniem).

use std::path::PathBuf;
use std::sync::Arc;

use accounts_hub_contract::SecretString;
use async_trait::async_trait;
use core_config_impl::FileConfigStore;
use sessions_contract::{PrivacyTag, SessionCatalog, SessionId};
use transfer_contract::{
    Category, CollisionResolution as TCollision, ExportRequest as TExportRequest,
    ExportScope as TScope, ImportMode as TMode, ImportOptions, ItemDiff as TDiff, ItemRef,
    ItemState, ModeMap, Selection, Transfer, TransferError, Warning,
};
use transfer_impl::ZipTransfer;

use app_api::dto::{
    CollisionResolution, DryRunItem, DryRunKind, ExportRequest, ExportResult, ImportMode,
    ImportRequest, ImportResult, InspectResult, ItemDiff, PackageManifestSummary, SecretInput,
};
use app_api::error::{AppError, ErrorCode};
use app_api::ports::{ShellPort, TransferPort};

/// Adapter modułu `transfer`.
pub struct TransferAdapter {
    transfer: Arc<ZipTransfer>,
    shell: Arc<dyn ShellPort>,
    sessions: Arc<dyn SessionCatalog>,
    config: Arc<FileConfigStore>,
}

/// Błąd modułu → błąd komendy (komunikat PL z modułu).
pub fn transfer_error(e: TransferError) -> AppError {
    use TransferError as E;
    let code = match &e {
        E::NotFound { .. } => ErrorCode::NotFound,
        E::SecretsNotAllowed | E::SecretDetected { .. } | E::EncryptionRequired { .. } => {
            ErrorCode::Forbidden
        }
        E::Io { .. } | E::Port { .. } => ErrorCode::Storage,
        _ => ErrorCode::InvalidInput,
    };
    AppError::new(code, format!("Import/eksport: {e}"))
}

fn secret(input: &SecretInput) -> SecretString {
    SecretString::from(input.expose())
}

async fn blocking<T: Send + 'static>(
    f: impl FnOnce() -> Result<T, AppError> + Send + 'static,
) -> Result<T, AppError> {
    tokio::task::spawn_blocking(f)
        .await
        .map_err(|e| AppError::internal(format!("zadanie transferu: {e}")))?
}

fn stamp() -> String {
    chrono::Local::now().format("%Y-%m-%d-%H%M").to_string()
}

fn kind_of(item: &ItemRef) -> DryRunKind {
    match item.category() {
        Category::Sessions => DryRunKind::Session,
        Category::Personas => DryRunKind::Persona,
        Category::Casts => DryRunKind::Cast,
        _ => DryRunKind::Config,
    }
}

fn key_of(item: &ItemRef) -> String {
    match item {
        ItemRef::Session { id } => id.to_string(),
        other => other.to_string(),
    }
}

fn diff_of(state: ItemState) -> ItemDiff {
    match state {
        ItemState::New => ItemDiff::New,
        ItemState::Same => ItemDiff::Same,
        ItemState::Changed => ItemDiff::Changed,
        ItemState::Collision => ItemDiff::Collision,
    }
}

fn warning_text(w: &Warning) -> String {
    match w {
        Warning::UnknownEntry { path } => format!("Pominięto nieznany wpis „{path}”."),
        Warning::KernelKeysSkipped { name, keys } => format!(
            "„{name}”: pominięto klucze polityk Jądra ({}) — zmienia je tylko Broker.",
            keys.join(", ")
        ),
        Warning::MachineOverlaySkipped { name } => {
            format!("Pominięto nakładkę innej maszyny „{name}”.")
        }
        Warning::HardwareClassDiffers { package, local } => {
            format!("Nakładka z maszyny klasy „{package}” (ta maszyna: „{local}”).")
        }
        Warning::PrivateSession { id, skipped } if *skipped => {
            format!("Sesja prywatna {id} pominięta (wymaga jawnego wyboru i hasła).")
        }
        Warning::PrivateSession { id, .. } => format!("Zaimportowano sesję prywatną {id}."),
        Warning::CategoryUnavailable { category } => format!(
            "Kategoria „{}” niedostępna w tej instalacji — pominięta.",
            category.key()
        ),
        Warning::NotMergeable { name } => {
            format!("Dokumentu „{name}” nie da się scalić — zostaje wersja lokalna.")
        }
        Warning::SecretStoreUnavailable => {
            "Magazyn sekretów niedostępny — strażnik działa tylko na wzorcach.".into()
        }
        Warning::Redacted { path, count } => {
            format!("„{path}”: zredagowano {count} ciągów wyglądających na sekrety.")
        }
        Warning::SnapshotUnencrypted => {
            "Snapshot przed importem nie jest zaszyfrowany (brak klucza maszyny).".into()
        }
    }
}

impl TransferAdapter {
    /// Adapter nad modułem `transfer`, dialogami powłoki, katalogiem sesji i konfiguracją.
    pub fn new(
        transfer: Arc<ZipTransfer>,
        shell: Arc<dyn ShellPort>,
        sessions: Arc<dyn SessionCatalog>,
        config: Arc<FileConfigStore>,
    ) -> Self {
        Self {
            transfer,
            shell,
            sessions,
            config,
        }
    }

    async fn save_path(&self, suggested: String) -> Result<Option<PathBuf>, AppError> {
        let shell = self.shell.clone();
        blocking(move || shell.pick_save_path(&suggested)).await
    }

    async fn run_export(&self, request: TExportRequest) -> Result<ExportResult, AppError> {
        let transfer = self.transfer.clone();
        let report = blocking(move || transfer.export(&request).map_err(transfer_error)).await?;
        Ok(ExportResult::Saved {
            path: report.path.to_string_lossy().into_owned(),
            files: report.manifest.content.len() as u64,
            bytes: report.file_bytes,
        })
    }

    /// Konfiguracja zmieniona na dysku (import/rollback) → przeładowanie warstw.
    async fn reload_config(&self) {
        if let Err(e) = self.config.reload().await {
            tracing::warn!(error = %e, "przeładowanie konfiguracji po imporcie nie powiodło się");
        }
    }
}

#[async_trait]
impl TransferPort for TransferAdapter {
    async fn export(&self, request: ExportRequest) -> Result<ExportResult, AppError> {
        let Some(dest) = self.save_path(format!("alfa-{}.alfa", stamp())).await? else {
            return Ok(ExportResult::Cancelled);
        };
        let ids: Vec<SessionId> = request.scope.sessions.iter().map(SessionId::new).collect();
        let scope = TScope {
            config_common: request.scope.config_common,
            personas: request.scope.personas,
            casts: request.scope.casts,
            sessions: if ids.is_empty() {
                Selection::None
            } else {
                Selection::Only(ids)
            },
            artifacts: request.scope.artifacts,
            logs: request.scope.logs,
            config_machine: request.scope.config_machine,
            ..TScope::default()
        };
        let mut req = TExportRequest::new(scope, dest);
        req.password = request.password.as_ref().map(secret);
        self.run_export(req).await
    }

    async fn export_session(&self, session: &SessionId) -> Result<ExportResult, AppError> {
        let meta = self.sessions.session(session).map_err(AppError::from)?;
        if meta.privacy != PrivacyTag::Normal {
            return Err(AppError::forbidden(
                "Sesja prywatna: eksport tylko jawnie, w paczce zaszyfrowanej hasłem \
                 (Ustawienia → Import i eksport).",
            ));
        }
        let name: String = meta
            .title
            .chars()
            .filter(|c| c.is_alphanumeric() || *c == ' ' || *c == '-')
            .take(40)
            .collect();
        let suggested = format!("{} {}.alfa", name.trim(), stamp());
        let Some(dest) = self.save_path(suggested).await? else {
            return Ok(ExportResult::Cancelled);
        };
        let scope = TScope {
            config_common: false,
            personas: false,
            casts: false,
            rules: false,
            skills: false,
            sessions: Selection::Only(vec![session.clone()]),
            ..TScope::default()
        };
        self.run_export(TExportRequest::new(scope, dest)).await
    }

    async fn export_secrets(&self, password: SecretInput) -> Result<ExportResult, AppError> {
        let password = secret(&password);
        transfer_contract::validate_password(&password).map_err(transfer_error)?;
        let suggested = format!("alfa-sekrety-{}.alfa", stamp());
        let Some(dest) = self.save_path(suggested).await? else {
            return Ok(ExportResult::Cancelled);
        };
        let transfer = self.transfer.clone();
        let report = blocking(move || {
            transfer
                .export_secrets(&dest, &password)
                .map_err(transfer_error)
        })
        .await?;
        Ok(ExportResult::Saved {
            path: report.path.to_string_lossy().into_owned(),
            files: report.manifest.content.len() as u64,
            bytes: report.file_bytes,
        })
    }

    async fn inspect(
        &self,
        password: Option<SecretInput>,
        path: Option<String>,
    ) -> Result<InspectResult, AppError> {
        let path = match path {
            Some(p) => PathBuf::from(p),
            None => {
                let shell = self.shell.clone();
                match blocking(move || shell.pick_open_path()).await? {
                    Some(p) => p,
                    None => return Ok(InspectResult::Cancelled),
                }
            }
        };
        let options = ImportOptions {
            password: password.as_ref().map(secret),
            ..ImportOptions::default()
        };
        let transfer = self.transfer.clone();
        let target = path.clone();
        let inspected = blocking(move || Ok(transfer.inspect(&target, &options))).await?;
        let shown = path.to_string_lossy().into_owned();
        let inspection = match inspected {
            Ok(i) => i,
            Err(TransferError::PasswordRequired) => {
                return Ok(InspectResult::NeedsPassword { path: shown });
            }
            Err(e) => return Err(transfer_error(e)),
        };
        let manifest = &inspection.manifest;
        Ok(InspectResult::Inspected {
            path: shown,
            manifest: PackageManifestSummary {
                schema_version: manifest.schema_version.to_string(),
                app_version: manifest.app_version.to_string(),
                created_at: app_api::dto::iso(manifest.created_at),
                source_machine: manifest.source_machine.name.clone(),
                encrypted: manifest.encryption.is_some(),
            },
            items: inspection.report.items.iter().map(item).collect(),
            warnings: inspection
                .report
                .warnings
                .iter()
                .map(warning_text)
                .collect(),
            migrations: inspection
                .report
                .migrations
                .iter()
                .map(|m| format!("{}: {} → {} ({})", m.entity, m.from, m.to, m.count))
                .collect(),
        })
    }

    async fn import(&self, request: ImportRequest) -> Result<ImportResult, AppError> {
        let mode = match request.mode {
            ImportMode::Add => TMode::Add,
            ImportMode::Merge => TMode::Merge,
            ImportMode::Replace => TMode::Replace,
        };
        let resolutions = request
            .resolutions
            .iter()
            .map(|(id, r)| {
                let r = match r {
                    CollisionResolution::KeepLocal => TCollision::Skip,
                    CollisionResolution::TakeImported => TCollision::Replace,
                    CollisionResolution::KeepBoth => TCollision::Copy,
                };
                (SessionId::new(id.as_str()), r)
            })
            .collect();
        let options = ImportOptions {
            password: request.password.as_ref().map(secret),
            modes: ModeMap::all(mode),
            resolutions,
            ..ImportOptions::default()
        };
        let transfer = self.transfer.clone();
        let path = PathBuf::from(&request.path);
        let report =
            blocking(move || transfer.import(&path, &options).map_err(transfer_error)).await?;
        self.reload_config().await;
        Ok(ImportResult {
            snapshot_id: report.snapshot.unwrap_or_default(),
            imported: report.added + report.merged + report.replaced + report.copied,
            skipped: report.skipped + report.failed,
        })
    }

    async fn rollback(&self, snapshot: &str) -> Result<(), AppError> {
        let transfer = self.transfer.clone();
        let id = snapshot.to_owned();
        blocking(move || transfer.rollback(&id).map(|_| ()).map_err(transfer_error)).await?;
        self.reload_config().await;
        Ok(())
    }
}

fn item(d: &TDiff) -> DryRunItem {
    DryRunItem {
        key: key_of(&d.item),
        kind: kind_of(&d.item),
        label: d.detail.clone().unwrap_or_else(|| d.item.to_string()),
        diff: diff_of(d.state),
    }
}
