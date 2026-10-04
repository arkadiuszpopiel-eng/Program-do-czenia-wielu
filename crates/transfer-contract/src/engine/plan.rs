//! Plan importu = dry-run: stan każdego elementu względem lokalnego, czynność wg trybu
//! i rozstrzygnięć kolizji, ostrzeżenia, migracje. Nic nie zapisuje (poza pobraniem nowych
//! identyfikatorów kopii ze źródła identyfikatorów).

use sessions_contract::{PrivacyTag, SessionId};

use crate::docmerge::{DocFormat, format_of, strip_kernel_keys};
use crate::engine::sessions::diff_sessions;
use crate::engine::{Engine, read_package_session};
use crate::error::TransferError;
use crate::manifest::{PackageKind, sha256_hex};
use crate::paths::{EntryKind, SESSION_FILE, TURNS_FILE, classify};
use crate::ports::PackageSource;
use crate::report::{DryRunReport, ItemDiff, ItemRef, ItemState, PlannedAction, Warning};
use crate::scope::{CancelToken, Category, CollisionResolution, ImportMode, ImportOptions};

/// Krok planu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Step {
    /// Element z paczki.
    pub item: ItemRef,
    /// Element lokalny, którego dotyczy zapis (inny niż `item` dla kopii i nakładki maszyny).
    pub target: ItemRef,
    /// Czynność.
    pub action: PlannedAction,
    /// Paczka jest kontynuacją lokalnej sesji (scalanie przestawia aktywny liść).
    pub fast_forward: bool,
}

/// Plan importu (wynik dry-run + kroki do wykonania).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportPlan {
    /// Raport dla UI.
    pub report: DryRunReport,
    /// Rodzaj paczki.
    pub kind: PackageKind,
    steps: Vec<Step>,
}

impl ImportPlan {
    /// Kroki.
    pub fn steps(&self) -> &[Step] {
        &self.steps
    }
}

fn skip(reason: &str) -> PlannedAction {
    PlannedAction::Skip {
        reason: reason.to_owned(),
    }
}

/// Czynność dla elementu istniejącego lokalnie i różnego od paczki.
fn by_mode(mode: ImportMode) -> PlannedAction {
    match mode {
        ImportMode::Add => skip("istnieje lokalnie (tryb „dodaj”)"),
        ImportMode::Merge => PlannedAction::Merge,
        ImportMode::Replace => PlannedAction::Replace,
    }
}

fn push(plan: &mut ImportPlan, target: ItemRef, diff: ItemDiff) {
    plan.steps.push(Step {
        item: diff.item.clone(),
        target,
        action: diff.action.clone(),
        fast_forward: false,
    });
    plan.report.items.push(diff);
}

impl Engine<'_> {
    /// Dry-run: plan importu paczki.
    pub fn plan(
        &self,
        source: &mut dyn PackageSource,
        options: &ImportOptions,
    ) -> Result<ImportPlan, TransferError> {
        let manifest = source.manifest().clone();
        // Paczka sekretów ze starszej wersji — czytelna odmowa (sekrety tylko w Credential Manager).
        if manifest.kind == PackageKind::Secrets {
            return Err(TransferError::SecretsNotAllowed);
        }
        let mut plan = ImportPlan {
            report: DryRunReport::default(),
            kind: manifest.kind,
            steps: Vec::new(),
        };
        plan.report.migrations = source.migrations();
        for entry in &manifest.content {
            CancelToken::check(options.cancel.as_ref())?;
            match classify(&entry.path) {
                EntryKind::Document { category, name } => {
                    self.plan_document(source, options, &mut plan, category, &name, entry.bytes)?;
                }
                EntryKind::SessionFile { id, file } if file == SESSION_FILE => {
                    let bytes = manifest
                        .entry(&crate::paths::session_path(&id, TURNS_FILE))
                        .map_or(0, |e| e.bytes);
                    self.plan_session(source, options, &mut plan, &id, entry.bytes + bytes)?;
                }
                EntryKind::SessionFile { file, .. } if file == TURNS_FILE => {}
                // Sekcja sekretów w zwykłej paczce (starsza wersja / spreparowana) — pominięta.
                EntryKind::Secrets => {
                    if !plan.report.warnings.contains(&Warning::SecretsSkipped) {
                        plan.report.warnings.push(Warning::SecretsSkipped);
                    }
                }
                EntryKind::Rollback if manifest.kind == PackageKind::Snapshot => {}
                _ => plan.report.warnings.push(Warning::UnknownEntry {
                    path: entry.path.clone(),
                }),
            }
        }
        Ok(plan)
    }

    fn plan_document(
        &self,
        source: &mut dyn PackageSource,
        options: &ImportOptions,
        plan: &mut ImportPlan,
        category: Category,
        name: &str,
        bytes: u64,
    ) -> Result<(), TransferError> {
        let item = ItemRef::Document {
            category,
            name: name.to_owned(),
        };
        let Some(store) = self.ports().store(category) else {
            if !plan
                .report
                .warnings
                .contains(&Warning::CategoryUnavailable { category })
            {
                plan.report
                    .warnings
                    .push(Warning::CategoryUnavailable { category });
            }
            push(
                plan,
                item.clone(),
                ItemDiff {
                    item,
                    state: ItemState::New,
                    action: skip("brak modułu dla kategorii"),
                    bytes,
                    detail: None,
                },
            );
            return Ok(());
        };
        let mut local_name = name.to_owned();
        if category == Category::ConfigMachine {
            if !options.include_machine_overlay {
                plan.report.warnings.push(Warning::MachineOverlaySkipped {
                    name: name.to_owned(),
                });
                push(
                    plan,
                    item.clone(),
                    ItemDiff {
                        item,
                        state: ItemState::New,
                        action: skip("nakładka maszyny tylko na wyraźne życzenie"),
                        bytes,
                        detail: None,
                    },
                );
                return Ok(());
            }
            let (pkg_class, local_class) = (
                source.manifest().source_machine.hw_class.clone(),
                self.ports().machine.hw_class.clone(),
            );
            if pkg_class != local_class {
                plan.report.warnings.push(Warning::HardwareClassDiffers {
                    package: pkg_class,
                    local: local_class,
                });
            }
            let (src_id, own_id) = (
                source.manifest().source_machine.id.clone(),
                self.ports().machine.id.clone(),
            );
            if !own_id.is_empty() && name == format!("{src_id}.toml") {
                local_name = format!("{own_id}.toml");
            }
        }
        let mut pkg = source
            .read(&crate::paths::document_path(category, name))?
            .ok_or_else(|| TransferError::corrupt(format!("brak dokumentu {name}")))?;
        if matches!(category, Category::ConfigCommon | Category::ConfigMachine)
            && format_of(name) == DocFormat::Toml
        {
            let (stripped, removed) = strip_kernel_keys(name, &pkg)?;
            if !removed.is_empty() {
                plan.report.warnings.push(Warning::KernelKeysSkipped {
                    name: name.to_owned(),
                    keys: removed,
                });
            }
            pkg = stripped;
        }
        let target = ItemRef::Document {
            category,
            name: local_name.clone(),
        };
        let (state, action) = match store.read(&local_name)? {
            None => (ItemState::New, PlannedAction::Add),
            Some(local) if sha256_hex(&local) == sha256_hex(&pkg) => {
                (ItemState::Same, PlannedAction::Keep)
            }
            Some(_) => {
                let mode = options.modes.mode_for(category);
                let action = by_mode(mode);
                if action == PlannedAction::Merge && format_of(name) == DocFormat::Opaque {
                    plan.report.warnings.push(Warning::NotMergeable {
                        name: name.to_owned(),
                    });
                    (
                        ItemState::Changed,
                        skip("formatu nie da się scalić — użyj „zastąp”"),
                    )
                } else {
                    (ItemState::Changed, action)
                }
            }
        };
        push(
            plan,
            target,
            ItemDiff {
                item,
                state,
                action,
                bytes,
                detail: None,
            },
        );
        Ok(())
    }

    fn plan_session(
        &self,
        source: &mut dyn PackageSource,
        options: &ImportOptions,
        plan: &mut ImportPlan,
        id: &SessionId,
        bytes: u64,
    ) -> Result<(), TransferError> {
        let item = ItemRef::Session { id: id.clone() };
        if self.ports().sessions.is_none() {
            let warning = Warning::CategoryUnavailable {
                category: Category::Sessions,
            };
            if !plan.report.warnings.contains(&warning) {
                plan.report.warnings.push(warning);
            }
            push(
                plan,
                item.clone(),
                ItemDiff {
                    item,
                    state: ItemState::New,
                    action: skip("brak modułu sesji"),
                    bytes,
                    detail: None,
                },
            );
            return Ok(());
        }
        let (pkg, steps) = read_package_session(self.ports(), source, id)?;
        for s in steps {
            match plan
                .report
                .migrations
                .iter_mut()
                .find(|m| m.entity == s.entity && m.from == s.from)
            {
                Some(m) => m.count += s.count,
                None => plan.report.migrations.push(s),
            }
        }
        if pkg.meta.privacy != PrivacyTag::Normal {
            plan.report.warnings.push(Warning::PrivateSession {
                id: id.clone(),
                skipped: false,
            });
        }
        let detail = Some(pkg.meta.title.clone());
        let resolution = options.resolutions.get(id).copied();
        let Some(local) = self.local_session(id)? else {
            let action = if resolution == Some(CollisionResolution::Skip) {
                skip("pominięta przez użytkownika")
            } else {
                PlannedAction::Add
            };
            push(
                plan,
                item.clone(),
                ItemDiff {
                    item,
                    state: ItemState::New,
                    action,
                    bytes,
                    detail,
                },
            );
            return Ok(());
        };
        let diff = diff_sessions(&local, &pkg);
        let detail = Some(format!("{} — {}", pkg.meta.title, diff.detail));
        if diff.state == ItemState::Same {
            push(
                plan,
                item.clone(),
                ItemDiff {
                    item,
                    state: ItemState::Same,
                    action: PlannedAction::Keep,
                    bytes,
                    detail,
                },
            );
            return Ok(());
        }
        let action = match resolution {
            Some(CollisionResolution::Merge) => PlannedAction::Merge,
            Some(CollisionResolution::Replace) => PlannedAction::Replace,
            Some(CollisionResolution::Skip) => skip("pominięta przez użytkownika"),
            Some(CollisionResolution::Copy) => {
                let new_id = self.ports().ids.new_session_id();
                plan.report.id_map.insert(id.clone(), new_id.clone());
                PlannedAction::Copy { new_id }
            }
            None => by_mode(options.modes.mode_for(Category::Sessions)),
        };
        let target = match &action {
            PlannedAction::Copy { new_id } => ItemRef::Session { id: new_id.clone() },
            _ => item.clone(),
        };
        push(
            plan,
            target,
            ItemDiff {
                item,
                state: diff.state,
                action,
                bytes,
                detail,
            },
        );
        if let Some(last) = plan.steps.last_mut() {
            last.fast_forward = diff.fast_forward;
        }
        Ok(())
    }
}
