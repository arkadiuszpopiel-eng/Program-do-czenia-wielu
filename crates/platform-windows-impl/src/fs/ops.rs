//! Operacje plikowe niskiego poziomu (przenośne, na `std::fs`) używane przez `WinFs`.
//! Część specyficzna dla Windows (przeniesienie bez nadpisania, Kosz) jest w `native`/`recycle`.

use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::SystemTime;

use platform_contract::PlatformError;

use super::guard::DenyPolicy;
use crate::error::from_io;

static TMP_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Odcisk pliku po operacji — cofnięcie odmawia, jeśli plik zmieniono później.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Fingerprint {
    len: u64,
    modified: Option<SystemTime>,
}

/// Odcisk wpisu (`None` = nie istnieje lub to katalog).
pub(crate) fn fingerprint(path: &Path) -> Option<Fingerprint> {
    let meta = fs::symlink_metadata(path).ok()?;
    meta.is_file().then(|| Fingerprint {
        len: meta.len(),
        modified: meta.modified().ok(),
    })
}

/// Unikalna nazwa pliku tymczasowego obok `path` (ten sam wolumin → atomowy `rename`).
pub(crate) fn sibling_tmp(path: &Path) -> PathBuf {
    let n = TMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    let name = path
        .file_name()
        .map_or_else(|| "plik".into(), |n| n.to_string_lossy().into_owned());
    path.with_file_name(format!(".{name}.alfa-{}-{n}.tmp", std::process::id()))
}

/// Tworzy brakujące katalogi nadrzędne; zwraca utworzone (od najpłytszego).
pub(crate) fn create_parents(path: &Path) -> io::Result<Vec<PathBuf>> {
    let mut missing = Vec::new();
    let mut cursor = path.parent();
    while let Some(dir) = cursor {
        if fs::symlink_metadata(dir).is_ok() {
            break;
        }
        missing.push(dir.to_path_buf());
        cursor = dir.parent();
    }
    missing.reverse();
    for dir in &missing {
        if let Err(e) = fs::create_dir(dir) {
            remove_created_dirs(&missing);
            return Err(e);
        }
    }
    Ok(missing)
}

/// Usuwa katalogi utworzone przez `create_parents` (tylko puste; od najgłębszego).
pub(crate) fn remove_created_dirs(dirs: &[PathBuf]) {
    for dir in dirs.iter().rev() {
        let _ = fs::remove_dir(dir);
    }
}

/// Zapis atomowy: plik tymczasowy obok celu, `sync_all`, `rename` nad celem.
pub(crate) fn write_atomic_file(path: &Path, data: &[u8]) -> io::Result<()> {
    let tmp = sibling_tmp(path);
    let result = (|| {
        let mut file = fs::File::create_new(&tmp)?;
        file.write_all(data)?;
        file.sync_all()?;
        drop(file);
        fs::rename(&tmp, path)
    })();
    if result.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    result
}

/// Przywraca kopię zapasową nad `path` (kopiowanie do pliku tymczasowego + `rename`).
pub(crate) fn restore_backup(backup: &Path, path: &Path) -> io::Result<()> {
    let tmp = sibling_tmp(path);
    let result = fs::copy(backup, &tmp).and_then(|_| fs::rename(&tmp, path));
    if result.is_err() {
        let _ = fs::remove_file(&tmp);
    } else {
        let _ = fs::remove_file(backup);
    }
    result
}

/// Kopiuje plik; cel nie może istnieć (rezerwacja przez `create_new`, potem `fs::copy`
/// — na Windows `CopyFileExW`, zachowuje atrybuty i znaczniki czasu).
fn copy_file_no_replace(from: &Path, to: &Path) -> io::Result<()> {
    drop(fs::File::create_new(to)?);
    fs::copy(from, to).map(|_| ()).inspect_err(|_| {
        let _ = fs::remove_file(to);
    })
}

/// Kopiuje plik lub drzewo katalogów. Każdy wpis drzewa przechodzi przez deny-listę
/// (kopia czyta treść); dowiązania wewnątrz drzewa są odrzucane. Błąd = wycofanie kopii.
pub(crate) fn copy_entry(
    from: &Path,
    to: &Path,
    policy: &DenyPolicy,
    user_from: &Path,
) -> Result<(), PlatformError> {
    let meta = fs::metadata(from).map_err(|e| from_io(&e, user_from))?;
    if !meta.is_dir() {
        return copy_file_no_replace(from, to).map_err(|e| from_io(&e, user_from));
    }
    fs::create_dir(to).map_err(|e| from_io(&e, user_from))?;
    copy_tree(from, to, policy, user_from).inspect_err(|_| {
        let _ = fs::remove_dir_all(to);
    })
}

fn copy_tree(
    from: &Path,
    to: &Path,
    policy: &DenyPolicy,
    user_from: &Path,
) -> Result<(), PlatformError> {
    let entries = fs::read_dir(from).map_err(|e| from_io(&e, user_from))?;
    for entry in entries {
        let entry = entry.map_err(|e| from_io(&e, user_from))?;
        let source = entry.path();
        if policy.is_denied(&source) {
            return Err(PlatformError::Denylisted(source));
        }
        let kind = entry.file_type().map_err(|e| from_io(&e, &source))?;
        let target = to.join(entry.file_name());
        if kind.is_symlink() {
            return Err(PlatformError::Unsupported(format!(
                "{}: kopiowanie dowiązań w drzewie katalogów",
                source.display()
            )));
        } else if kind.is_dir() {
            fs::create_dir(&target).map_err(|e| from_io(&e, &source))?;
            copy_tree(&source, &target, policy, user_from)?;
        } else {
            copy_file_no_replace(&source, &target).map_err(|e| from_io(&e, &source))?;
        }
    }
    Ok(())
}

/// Usuwa wpis: katalog rekurencyjnie (bez podążania za dowiązaniami), plik lub samo dowiązanie.
pub(crate) fn remove_entry(path: &Path) -> io::Result<()> {
    let meta = fs::symlink_metadata(path)?;
    if meta.is_symlink() {
        // Dowiązanie do katalogu na Windows usuwa się jak katalog, na Unix jak plik.
        fs::remove_file(path).or_else(|_| fs::remove_dir(path))
    } else if meta.is_dir() {
        fs::remove_dir_all(path)
    } else {
        fs::remove_file(path)
    }
}

/// Czy dwie ścieżki operacji wskazują ten sam wpis (Windows: bez rozróżniania wielkości liter).
pub(crate) fn same_entry(a: &Path, b: &Path) -> bool {
    if cfg!(windows) {
        a.to_string_lossy().to_lowercase() == b.to_string_lossy().to_lowercase()
    } else {
        a == b
    }
}

/// Przeniesienie bez nadpisywania celu.
pub(crate) fn move_no_replace(from: &Path, to: &Path) -> io::Result<()> {
    #[cfg(windows)]
    {
        super::native::move_no_replace(from, to)
    }
    #[cfg(not(windows))]
    {
        // Brak `renameat2(RENAME_NOREPLACE)` w std: sprawdzenie + rename (wąskie okno wyścigu;
        // platforma docelowa to Windows, gdzie `MoveFileExW` bez REPLACE_EXISTING jest atomowe).
        if fs::symlink_metadata(to).is_ok() && !same_entry(from, to) {
            return Err(io::Error::from(io::ErrorKind::AlreadyExists));
        }
        fs::rename(from, to)
    }
}
