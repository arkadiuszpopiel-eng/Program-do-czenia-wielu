//! Dziennik cofnięć operacji FS w pamięci procesu (trwały dziennik to moduł `undo-journal`, F3).

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use platform_contract::{PlatformError, UndoToken};

use super::ops::{self, Fingerprint};
use crate::error::from_io;

/// Jak cofnąć operację.
#[derive(Debug)]
pub(crate) enum UndoAction {
    /// Przywróć poprzednią zawartość z kopii (`None` = pliku nie było → usuń).
    RestoreContent {
        path: PathBuf,
        backup: Option<PathBuf>,
        previous_modified: Option<SystemTime>,
        created_dirs: Vec<PathBuf>,
        after: Option<Fingerprint>,
    },
    /// Usuń kopię.
    RemoveCopy {
        path: PathBuf,
        created_dirs: Vec<PathBuf>,
        after: Option<Fingerprint>,
    },
    /// Przenieś z powrotem.
    MoveBack {
        current: PathBuf,
        original: PathBuf,
        created_dirs: Vec<PathBuf>,
    },
    /// Przywróć z Kosza (`recycled` = `$R…`, obok leży `$I…` z metadanymi).
    RestoreFromBin {
        recycled: PathBuf,
        original: PathBuf,
    },
    /// Nic (przeniesienie na siebie).
    Noop,
}

/// Dziennik: token → akcja cofnięcia (tokeny jednorazowe, rosnące).
#[derive(Debug, Default)]
pub(crate) struct Journal {
    actions: BTreeMap<u64, UndoAction>,
    next: u64,
}

impl Journal {
    /// Rezerwuje nowy token (używany też w nazwie kopii zapasowej).
    pub(crate) fn allocate(&mut self) -> u64 {
        self.next += 1;
        self.next
    }

    /// Zapisuje akcję pod tokenem.
    pub(crate) fn insert(&mut self, token: u64, action: UndoAction) {
        self.actions.insert(token, action);
    }

    /// Wyjmuje akcję (token zużyty).
    pub(crate) fn take(&mut self, token: UndoToken) -> Result<UndoAction, PlatformError> {
        self.actions
            .remove(&token.0)
            .ok_or(PlatformError::UnknownUndoToken(token.0))
    }

    /// Liczba oczekujących tokenów.
    pub(crate) fn len(&self) -> usize {
        self.actions.len()
    }

    /// Wszystkie kopie zapasowe (do sprzątania).
    pub(crate) fn backups(&self) -> impl Iterator<Item = &PathBuf> {
        self.actions.values().filter_map(|a| match a {
            UndoAction::RestoreContent {
                backup: Some(b), ..
            } => Some(b),
            _ => None,
        })
    }
}

fn changed_since(path: &Path, after: Option<Fingerprint>) -> Result<(), PlatformError> {
    if ops::fingerprint(path) != after {
        return Err(PlatformError::NotReversible(format!(
            "{}: plik zmieniono po operacji — cofnięcie nadpisałoby nowsze zmiany",
            path.display()
        )));
    }
    Ok(())
}

/// Wykonuje cofnięcie. Przy błędzie akcja wraca do wywołującego (można ją ponowić).
pub(crate) fn apply(action: &UndoAction) -> Result<(), PlatformError> {
    match action {
        UndoAction::RestoreContent {
            path,
            backup,
            previous_modified,
            created_dirs,
            after,
        } => {
            changed_since(path, *after)?;
            match backup {
                Some(backup) => {
                    ops::restore_backup(backup, path).map_err(|e| from_io(&e, path))?;
                    // Przywrócony plik ma dawny czas modyfikacji (odcisk wcześniejszych operacji
                    // w dzienniku musi się zgadzać przy cofaniu LIFO).
                    if let Some(time) = previous_modified {
                        let _ = fs::File::options()
                            .write(true)
                            .open(path)
                            .and_then(|f| f.set_modified(*time));
                    }
                    Ok(())
                }
                None => {
                    fs::remove_file(path).map_err(|e| from_io(&e, path))?;
                    ops::remove_created_dirs(created_dirs);
                    Ok(())
                }
            }
        }
        UndoAction::RemoveCopy {
            path,
            created_dirs,
            after,
        } => {
            if after.is_some() {
                changed_since(path, *after)?;
            }
            ops::remove_entry(path).map_err(|e| from_io(&e, path))?;
            ops::remove_created_dirs(created_dirs);
            Ok(())
        }
        UndoAction::MoveBack {
            current,
            original,
            created_dirs,
        } => {
            ops::move_no_replace(current, original).map_err(|e| from_io(&e, original))?;
            ops::remove_created_dirs(created_dirs);
            Ok(())
        }
        UndoAction::RestoreFromBin { recycled, original } => {
            ops::move_no_replace(recycled, original).map_err(|e| from_io(&e, original))?;
            #[cfg(windows)]
            if let Some(meta) = super::recycle::recycle_metadata_sibling(recycled) {
                let _ = fs::remove_file(meta);
            }
            Ok(())
        }
        UndoAction::Noop => Ok(()),
    }
}
