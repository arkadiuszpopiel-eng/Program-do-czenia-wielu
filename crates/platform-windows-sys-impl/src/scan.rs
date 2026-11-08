//! Pełne przeskanowanie katalogu obserwacji (start, przepełnienie bufora, przeniesiony podkatalog):
//! `std::fs` bez podążania za dowiązaniami i junction (na Windows `is_symlink` obejmuje junction),
//! bez wchodzenia do katalogów z deny-listy, z limitem liczby plików.

use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use platform_contract::{FileStamp, WatchPolicy};

/// Stan ścieżki (bez podążania za dowiązaniem).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stat {
    /// Plik z odciskiem.
    File(FileStamp),
    /// Katalog.
    Dir,
    /// Nie istnieje (albo dowiązanie — nieobserwowane).
    Missing,
}

fn stamp_of(meta: &std::fs::Metadata) -> FileStamp {
    let modified_ms = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map_or(0, |d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX));
    FileStamp {
        len: meta.len(),
        modified_ms,
    }
}

/// Stan ścieżki teraz.
pub fn stat_path(path: &Path) -> Stat {
    match std::fs::symlink_metadata(path) {
        Ok(m) if m.file_type().is_symlink() => Stat::Missing,
        Ok(m) if m.is_dir() => Stat::Dir,
        Ok(m) => Stat::File(stamp_of(&m)),
        Err(_) => Stat::Missing,
    }
}

/// Pliki w `dir` (z podkatalogami, jeśli `recursive`), najwyżej `max`; `true` = przycięto.
/// Katalogi i pliki z deny-listy są pomijane bez odczytu zawartości.
pub fn scan_dir(
    dir: &Path,
    recursive: bool,
    policy: &WatchPolicy,
    max: usize,
) -> (Vec<(PathBuf, FileStamp)>, bool) {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(current) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&current) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if policy.is_denied(&path) {
                continue;
            }
            let Ok(kind) = entry.file_type() else {
                continue;
            };
            if kind.is_symlink() {
                continue;
            }
            if kind.is_dir() {
                if recursive {
                    stack.push(path);
                }
                continue;
            }
            if out.len() >= max {
                return (out, true);
            }
            if let Ok(meta) = entry.metadata() {
                out.push((path, stamp_of(&meta)));
            }
        }
    }
    (out, false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scan_skips_denied_and_links_and_caps() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        std::fs::create_dir_all(root.join("sub")).unwrap();
        std::fs::create_dir_all(root.join(".ssh")).unwrap();
        std::fs::write(root.join("a.txt"), b"a").unwrap();
        std::fs::write(root.join("sub/b.txt"), b"bb").unwrap();
        std::fs::write(root.join(".ssh/id_ed25519"), b"tajne").unwrap();
        std::fs::write(root.join(".npmrc"), b"token").unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink(root.join(".ssh"), root.join("skrot")).unwrap();
        let policy = WatchPolicy::baseline();
        let (flat, cut) = scan_dir(root, false, &policy, 100);
        assert!(!cut);
        assert_eq!(flat.len(), 1);
        assert_eq!(flat[0].0, root.join("a.txt"));
        assert_eq!(flat[0].1.len, 1);
        let (mut deep, _) = scan_dir(root, true, &policy, 100);
        deep.sort();
        let names: Vec<_> = deep.iter().map(|(p, _)| p.clone()).collect();
        assert_eq!(names, vec![root.join("a.txt"), root.join("sub/b.txt")]);
        let (capped, cut) = scan_dir(root, true, &policy, 1);
        assert!(cut && capped.len() == 1);
        assert_eq!(stat_path(&root.join("sub")), Stat::Dir);
        assert!(matches!(stat_path(&root.join("a.txt")), Stat::File(_)));
        assert_eq!(stat_path(&root.join("brak")), Stat::Missing);
        #[cfg(unix)]
        assert_eq!(stat_path(&root.join("skrot")), Stat::Missing);
    }
}
