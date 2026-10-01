//! Zapis planu per element (każdy element w całości albo wcale; sesja zastępowana — przy błędzie
//! przywracana z wersji sprzed zapisu) i rollback ze snapshotu.

use accounts_hub_contract::SecretName;
use sessions_contract::{PortableSession, SessionError, SessionId, SessionQuery, unique_dir_name};

use crate::docmerge::{DocFormat, format_of, merge_documents, strip_kernel_keys};
use crate::engine::plan::Step;
use crate::engine::sessions::merge_turns;
use crate::engine::{Engine, ImportPlan, RollbackData, decode_secrets, read_package_session};
use crate::error::TransferError;
use crate::paths::{EntryKind, ROLLBACK_PATH, SECRETS_PATH, SESSION_FILE, classify, document_path};
use crate::ports::PackageSource;
use crate::report::{ImportReport, ItemOutcome, ItemRef, Outcome, PlannedAction, RollbackReport};
use crate::scope::{CancelToken, Category};

impl Engine<'_> {
    /// Wykonuje plan; błędy elementów trafiają do raportu (element nietknięty), błąd anulowania
    /// przerywa między elementami.
    pub fn apply(
        &self,
        source: &mut dyn PackageSource,
        plan: &ImportPlan,
        cancel: Option<&CancelToken>,
    ) -> Result<ImportReport, TransferError> {
        let mut report = ImportReport {
            id_map: plan.report.id_map.clone(),
            warnings: plan.report.warnings.clone(),
            ..ImportReport::default()
        };
        for step in plan.steps() {
            if !step.action.writes() {
                report.skipped += 1;
                continue;
            }
            CancelToken::check(cancel)?;
            let result = match &step.item {
                ItemRef::Document { category, name } => {
                    self.apply_document(source, step, *category, name)
                }
                ItemRef::Session { id } => self.apply_session(source, step, id),
                ItemRef::Secret { name } => self.apply_secret(source, name),
            };
            let outcome = match result {
                Ok(()) => {
                    match step.action {
                        PlannedAction::Add => report.added += 1,
                        PlannedAction::Merge => report.merged += 1,
                        PlannedAction::Replace => report.replaced += 1,
                        PlannedAction::Copy { .. } => report.copied += 1,
                        PlannedAction::Keep | PlannedAction::Skip { .. } => {}
                    }
                    Outcome::Done {
                        action: step.action.clone(),
                    }
                }
                Err(e) => {
                    report.failed += 1;
                    Outcome::Failed {
                        reason: e.to_string(),
                    }
                }
            };
            report.items.push(ItemOutcome {
                item: step.item.clone(),
                outcome,
            });
        }
        Ok(report)
    }

    fn apply_document(
        &self,
        source: &mut dyn PackageSource,
        step: &Step,
        category: Category,
        name: &str,
    ) -> Result<(), TransferError> {
        let store = self
            .ports()
            .store(category)
            .ok_or_else(|| TransferError::Unsupported {
                category: category.key().to_owned(),
            })?;
        let ItemRef::Document {
            name: local_name, ..
        } = &step.target
        else {
            return Err(TransferError::invalid(name, "zły cel kroku"));
        };
        let mut pkg = source
            .read(&document_path(category, name))?
            .ok_or_else(|| TransferError::corrupt(format!("brak dokumentu {name}")))?;
        if matches!(category, Category::ConfigCommon | Category::ConfigMachine)
            && format_of(name) == DocFormat::Toml
        {
            pkg = strip_kernel_keys(name, &pkg)?.0;
        }
        let bytes = match (&step.action, store.read(local_name)?) {
            (PlannedAction::Merge, Some(local)) => merge_documents(name, &local, &pkg)?
                .ok_or_else(|| TransferError::invalid(name, "formatu nie da się scalić"))?,
            _ => pkg,
        };
        store.write(local_name, &bytes)
    }

    fn apply_session(
        &self,
        source: &mut dyn PackageSource,
        step: &Step,
        id: &SessionId,
    ) -> Result<(), TransferError> {
        let sessions = self.ports().sessions()?;
        let (pkg, _) = read_package_session(self.ports(), source, id)?;
        match &step.action {
            PlannedAction::Add => {
                sessions.adopt_session(pkg)?;
            }
            PlannedAction::Replace => {
                let previous = self.local_session(id)?;
                if previous.is_some() {
                    sessions.delete_session(id)?;
                }
                if let Err(e) = sessions.adopt_session(pkg) {
                    // Kompensacja: przywrócenie wersji sprzed zapisu (snapshot i tak istnieje).
                    if let Some(previous) = previous {
                        let _ = sessions.adopt_session(previous);
                    }
                    return Err(e.into());
                }
            }
            PlannedAction::Merge => self.merge_session(id, &pkg, step.fast_forward)?,
            PlannedAction::Copy { new_id } => {
                let copy = self.as_copy(pkg, new_id, &source.manifest().source_machine.name)?;
                sessions.adopt_session(copy)?;
            }
            PlannedAction::Keep | PlannedAction::Skip { .. } => {}
        }
        Ok(())
    }

    fn merge_session(
        &self,
        id: &SessionId,
        pkg: &PortableSession,
        fast_forward: bool,
    ) -> Result<(), TransferError> {
        let sessions = self.ports().sessions()?;
        let Some(local) = self.local_session(id)? else {
            sessions.adopt_session(pkg.clone())?;
            return Ok(());
        };
        let merged = merge_turns(&local.turns, &pkg.turns)?;
        sessions.import_turns(id, &merged.new_turns)?;
        for (turn, prefix) in merged.heard {
            match sessions.record_heard_prefix(id, turn, prefix) {
                Ok(_) | Err(SessionError::HeardPrefixAlreadyRecorded { .. }) => {}
                Err(e) => return Err(e.into()),
            }
        }
        if pkg.meta.tainted && !local.meta.tainted {
            sessions.mark_tainted(id)?;
        }
        if fast_forward
            && let Some(leaf) = pkg.active_leaf.and_then(|l| merged.map.get(&l).copied())
        {
            sessions.set_active_leaf(id, leaf)?;
        }
        Ok(())
    }

    /// Kopia z nowym identyfikatorem, tytułem „(import z …)” i unikalnym katalogiem roboczym.
    fn as_copy(
        &self,
        mut pkg: PortableSession,
        new_id: &SessionId,
        machine: &str,
    ) -> Result<PortableSession, TransferError> {
        let from = if machine.trim().is_empty() {
            "innej maszyny"
        } else {
            machine.trim()
        };
        pkg.meta.id = new_id.clone();
        pkg.meta.title = format!("{} (import z {from})", pkg.meta.title);
        let query = SessionQuery {
            include_archived: true,
            ..SessionQuery::default()
        };
        let mut taken: Vec<String> = self
            .ports()
            .sessions()?
            .list_sessions(&query)?
            .iter()
            .map(|s| s.meta.workdir_name())
            .collect();
        taken.extend(
            self.ports()
                .sessions()?
                .list_sessions(&SessionQuery {
                    trashed: true,
                    ..query
                })?
                .iter()
                .map(|s| s.meta.workdir_name()),
        );
        let base = pkg.meta.workdir_name();
        let unique = unique_dir_name(&format!("{base} (import)"), &taken);
        pkg.meta.workdir = pkg
            .meta
            .workdir
            .parent()
            .map_or_else(|| unique.clone().into(), |p| p.join(&unique));
        Ok(pkg)
    }

    fn apply_secret(
        &self,
        source: &mut dyn PackageSource,
        name: &str,
    ) -> Result<(), TransferError> {
        let store = self
            .ports()
            .secrets
            .as_ref()
            .ok_or_else(|| TransferError::Unsupported {
                category: Category::Secrets.key().to_owned(),
            })?;
        let bytes = zeroize::Zeroizing::new(source.read(SECRETS_PATH)?.unwrap_or_default());
        let secrets = decode_secrets(&bytes)?;
        let value = secrets.get(name).ok_or_else(|| TransferError::NotFound {
            what: format!("sekret {name}"),
        })?;
        let name = SecretName::new(name).map_err(|e| TransferError::invalid(SECRETS_PATH, e))?;
        store.put(&name, value)?;
        Ok(())
    }

    /// Rollback: przywraca elementy ze snapshotu (w całości) i usuwa elementy utworzone przez import.
    pub fn rollback(
        &self,
        snapshot: &mut dyn PackageSource,
    ) -> Result<RollbackReport, TransferError> {
        let data: RollbackData = match snapshot.read(ROLLBACK_PATH)? {
            Some(bytes) => serde_json::from_slice(&bytes)
                .map_err(|e| TransferError::invalid(ROLLBACK_PATH, e))?,
            None => RollbackData::default(),
        };
        let mut report = RollbackReport::default();
        let paths: Vec<String> = snapshot
            .manifest()
            .content
            .iter()
            .map(|e| e.path.clone())
            .collect();
        for path in paths {
            let (item, result) = match classify(&path) {
                EntryKind::Document { category, name } => {
                    let r = self.restore_document(snapshot, category, &name, &path);
                    (ItemRef::Document { category, name }, r)
                }
                EntryKind::SessionFile { id, file } if file == SESSION_FILE => {
                    let r = self.restore_session(snapshot, &id);
                    (ItemRef::Session { id }, r)
                }
                EntryKind::Secrets => {
                    let r = self.restore_secrets(snapshot);
                    (
                        ItemRef::Secret {
                            name: "*".to_owned(),
                        },
                        r,
                    )
                }
                _ => continue,
            };
            match result {
                Ok(()) => report.restored += 1,
                Err(e) => report.failed.push(ItemOutcome {
                    item,
                    outcome: Outcome::Failed {
                        reason: e.to_string(),
                    },
                }),
            }
        }
        rollback_items(self, &data.created, &mut report);
        Ok(report)
    }

    fn restore_document(
        &self,
        snapshot: &mut dyn PackageSource,
        category: Category,
        name: &str,
        path: &str,
    ) -> Result<(), TransferError> {
        let store = self
            .ports()
            .store(category)
            .ok_or_else(|| TransferError::Unsupported {
                category: category.key().to_owned(),
            })?;
        let bytes = snapshot.read(path)?.unwrap_or_default();
        store.write(name, &bytes)
    }

    fn restore_session(
        &self,
        snapshot: &mut dyn PackageSource,
        id: &SessionId,
    ) -> Result<(), TransferError> {
        let sessions = self.ports().sessions()?;
        let (session, _) = read_package_session(self.ports(), snapshot, id)?;
        if self.local_session(id)?.is_some() {
            sessions.delete_session(id)?;
        }
        sessions.adopt_session(session)?;
        Ok(())
    }

    fn restore_secrets(&self, snapshot: &mut dyn PackageSource) -> Result<(), TransferError> {
        let store = self
            .ports()
            .secrets
            .as_ref()
            .ok_or_else(|| TransferError::Unsupported {
                category: Category::Secrets.key().to_owned(),
            })?;
        let bytes = zeroize::Zeroizing::new(snapshot.read(SECRETS_PATH)?.unwrap_or_default());
        for (name, value) in decode_secrets(&bytes)? {
            let name =
                SecretName::new(name).map_err(|e| TransferError::invalid(SECRETS_PATH, e))?;
            store.put(&name, &value)?;
        }
        Ok(())
    }
}

/// Usuwa elementy utworzone przez import (część rollbacku).
pub fn rollback_items(engine: &Engine<'_>, created: &[ItemRef], report: &mut RollbackReport) {
    for item in created {
        let result: Result<bool, TransferError> = match item {
            ItemRef::Document { category, name } => match engine.ports().store(*category) {
                Some(store) => store.remove(name),
                None => Ok(false),
            },
            ItemRef::Session { id } => {
                engine
                    .ports()
                    .sessions()
                    .and_then(|s| match s.delete_session(id) {
                        Ok(_) => Ok(true),
                        Err(SessionError::NotFound { .. }) => Ok(false),
                        Err(e) => Err(e.into()),
                    })
            }
            ItemRef::Secret { name } => {
                match (&engine.ports().secrets, SecretName::new(name.clone())) {
                    (Some(store), Ok(n)) => store.delete(&n).map_err(Into::into),
                    _ => Ok(false),
                }
            }
        };
        match result {
            Ok(true) => report.removed += 1,
            Ok(false) => {}
            Err(e) => report.failed.push(ItemOutcome {
                item: item.clone(),
                outcome: Outcome::Failed {
                    reason: e.to_string(),
                },
            }),
        }
    }
}
