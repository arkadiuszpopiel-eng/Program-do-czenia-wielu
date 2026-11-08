//! Nazwy plików paczek z znacznikiem czasu i rotacja (kopie zapasowe, snapshoty).

use std::path::{Path, PathBuf};

use transfer_contract::backup::{EXTENSION, rotation_victims, stamped_name};

/// Wolna ścieżka `<dir>/<prefiks><znacznik>.alfa` (przy kolizji znacznik +1 ms).
pub(crate) fn unique_path(
    dir: &Path,
    prefix: &str,
    mut at: chrono::DateTime<chrono::Utc>,
) -> (String, PathBuf) {
    loop {
        let stem = stamped_name(prefix, at);
        let path = dir.join(format!("{stem}.{EXTENSION}"));
        if !path.exists() {
            return (stem, path);
        }
        at += chrono::Duration::milliseconds(1);
    }
}

/// Rotacja: zostaje `keep` najnowszych plików z prefiksem; inne pliki nietknięte.
pub(crate) fn rotate(dir: &Path, prefix: &str, keep: usize) -> Vec<PathBuf> {
    let stems: Vec<String> = std::fs::read_dir(dir)
        .map(|entries| {
            entries
                .flatten()
                .filter_map(|e| {
                    e.file_name()
                        .to_str()
                        .and_then(|n| n.strip_suffix(&format!(".{EXTENSION}")).map(str::to_owned))
                })
                .collect()
        })
        .unwrap_or_default();
    rotation_victims(prefix, &stems, keep)
        .into_iter()
        .map(|stem| dir.join(format!("{stem}.{EXTENSION}")))
        .filter(|path| std::fs::remove_file(path).is_ok())
        .collect()
}
