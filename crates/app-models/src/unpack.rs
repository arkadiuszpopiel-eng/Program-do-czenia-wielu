//! Bezpieczne rozpakowanie archiwów ZIP (sidecary, paczki PyPI) — reguły jak dla paczek
//! aktualizacji (`updater-impl/src/install.rs`, `transfer`): ścieżki bez `..`/`\`/`:`/nazw urządzeń
//! (`updater_contract::validate_package_path`), bez dowiązań, bez duplikatów (także różniących się
//! wielkością liter — NTFS), limity liczby wpisów, rozmiaru wpisu i całości oraz stopnia kompresji
//! (zip-bomb); rzeczywista liczba bajtów nie może przekroczyć zadeklarowanej. Całe drzewo trafia
//! najpierw do katalogu roboczego obok celu, potem — atomowo przez `rename` — na miejsce.

use std::collections::{BTreeMap, BTreeSet};
use std::fs::File;
use std::io::{BufReader, Read, Write};
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};
use updater_contract::{PackageLimits, validate_package_path};
use zip::ZipArchive;

use crate::catalog::Pick;
use crate::fetch::hex;

/// Tryb pliku Unix: dowiązanie symboliczne.
const S_IFLNK: u32 = 0o120_000;
const S_IFMT: u32 = 0o170_000;
/// Prefiks katalogu roboczego (pomijany przy wykrywaniu instalacji).
pub const STAGING_PREFIX: &str = ".staging-";

/// Odrzucone archiwum.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unsafe(pub String);

impl std::fmt::Display for Unsafe {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "niebezpieczne albo uszkodzone archiwum: {}", self.0)
    }
}

fn bad(why: impl std::fmt::Display) -> Unsafe {
    Unsafe(why.to_string())
}

/// Plik wynikowy: ścieżka względna → SHA-256.
pub type Hashes = BTreeMap<String, String>;

struct Reader {
    archive: ZipArchive<BufReader<File>>,
    limits: PackageLimits,
    seen: BTreeSet<String>,
    total: u64,
}

impl Reader {
    fn open(path: &Path, limits: PackageLimits) -> Result<Self, Unsafe> {
        let file = File::open(path).map_err(|e| bad(format!("{}: {e}", path.display())))?;
        let archive = ZipArchive::new(BufReader::new(file)).map_err(bad)?;
        if archive.len() as u64 > limits.max_entries {
            return Err(bad(format!(
                "{} wpisów > {}",
                archive.len(),
                limits.max_entries
            )));
        }
        Ok(Self {
            archive,
            limits,
            seen: BTreeSet::new(),
            total: 0,
        })
    }

    /// Sprawdza nagłówek wpisu `i`; zwraca jego nazwę, czy to katalog i rozmiar.
    fn check(&mut self, i: usize) -> Result<(String, bool, u64), Unsafe> {
        let entry = self.archive.by_index_raw(i).map_err(bad)?;
        let name = entry.name().to_owned();
        validate_package_path(&name).map_err(bad)?;
        if !self.seen.insert(name.trim_end_matches('/').to_lowercase()) {
            return Err(bad(format!("powtórzony wpis „{name}”")));
        }
        if entry.unix_mode().is_some_and(|m| m & S_IFMT == S_IFLNK) {
            return Err(bad(format!("dowiązanie „{name}”")));
        }
        let (dir, size) = (entry.is_dir(), entry.size());
        if !dir {
            self.total = self
                .limits
                .admit(size, entry.compressed_size(), self.total)
                .map_err(bad)?;
        }
        Ok((name, dir, size))
    }

    /// Kopiuje wpis `i` (rozmiar `size` z nagłówka) do nowego pliku `out`; zwraca SHA-256.
    fn copy(&mut self, i: usize, size: u64, out: &Path) -> Result<String, Unsafe> {
        let entry = self.archive.by_index(i).map_err(bad)?;
        if let Some(parent) = out.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| bad(format!("{}: {e}", parent.display())))?;
        }
        let mut file = File::create_new(out).map_err(|e| bad(format!("{}: {e}", out.display())))?;
        let mut hasher = Sha256::new();
        let mut reader = entry.take(size.saturating_add(1));
        let mut buf = vec![0u8; 1 << 16];
        let mut copied = 0u64;
        loop {
            let n = reader.read(&mut buf).map_err(bad)?;
            if n == 0 {
                break;
            }
            copied += n as u64;
            if copied > size {
                return Err(bad(format!("wpis większy niż zadeklarowane {size} B")));
            }
            file.write_all(&buf[..n]).map_err(bad)?;
            hasher.update(&buf[..n]);
        }
        if copied != size {
            return Err(bad(format!("wpis: {copied} B zamiast {size} B")));
        }
        file.flush().map_err(bad)?;
        Ok(hex(&hasher.finalize()))
    }
}

/// Wybrane wpisy archiwum → `dir/<dest>` (każdy przez plik `.part`, hash sprawdzany z przypiętym).
/// Wszystkie nagłówki archiwum są sprawdzane (jeden zły wpis odrzuca całość).
pub fn extract_picks(
    archive: &Path,
    dir: &Path,
    picks: &[Pick],
    limits: PackageLimits,
) -> Result<Hashes, Unsafe> {
    let mut reader = Reader::open(archive, limits)?;
    let mut found: BTreeMap<String, (usize, u64)> = BTreeMap::new();
    for i in 0..reader.archive.len() {
        let (name, dir_entry, size) = reader.check(i)?;
        if !dir_entry && picks.iter().any(|p| p.member == name) {
            found.insert(name, (i, size));
        }
    }
    let mut out = Hashes::new();
    for pick in picks {
        validate_package_path(&pick.dest).map_err(bad)?;
        let (i, size) = *found
            .get(&pick.member)
            .ok_or_else(|| bad(format!("brak wpisu „{}”", pick.member)))?;
        let dest = dir.join(&pick.dest);
        let part = crate::fetch::part_path(&dest);
        let _ = std::fs::remove_file(&part);
        let sha = reader.copy(i, size, &part).inspect_err(|_| {
            let _ = std::fs::remove_file(&part);
        })?;
        if let Some(pin) = &pick.sha256
            && !pin.eq_ignore_ascii_case(&sha)
        {
            let _ = std::fs::remove_file(&part);
            return Err(bad(format!(
                "SHA-256 wpisu „{}”: {sha} ≠ {pin}",
                pick.member
            )));
        }
        std::fs::rename(&part, &dest).map_err(|e| bad(format!("{}: {e}", dest.display())))?;
        out.insert(pick.dest.clone(), sha);
    }
    Ok(out)
}

/// Katalog roboczy dla celu `target` (obok niego, ta sama partycja).
pub fn staging_dir(target: &Path) -> PathBuf {
    let name = target
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    target.with_file_name(format!("{STAGING_PREFIX}{name}"))
}

/// Całe drzewo archiwum (bez prefiksu `strip`; wpisy spoza niego pomijane) → `target`, atomowo.
/// Po rozpakowaniu muszą istnieć pliki `require`. Zwraca hashe wszystkich plików.
pub fn extract_tree(
    archive: &Path,
    target: &Path,
    strip: &str,
    require: &[String],
    limits: PackageLimits,
) -> Result<Hashes, Unsafe> {
    extract_trees(&[archive.to_path_buf()], target, strip, require, limits)
}

/// Jak [`extract_tree`] dla kilku archiwów rozpakowywanych do jednego drzewa (np. `llama-server`
/// CUDA + biblioteki `cudart` z osobnego archiwum wydania). Ten sam plik w dwóch archiwach
/// (także różniący się wielkością liter) odrzuca całość — nic nie jest nadpisywane po cichu.
pub fn extract_trees(
    archives: &[PathBuf],
    target: &Path,
    strip: &str,
    require: &[String],
    limits: PackageLimits,
) -> Result<Hashes, Unsafe> {
    if archives.is_empty() {
        return Err(bad("pozycja bez archiwum"));
    }
    let work = staging_dir(target);
    if work.exists() {
        std::fs::remove_dir_all(&work).map_err(bad)?;
    }
    std::fs::create_dir_all(&work).map_err(bad)?;
    let result = trees_into(archives, &work, strip, require, limits);
    let hashes = match result {
        Ok(h) => h,
        Err(e) => {
            let _ = std::fs::remove_dir_all(&work);
            return Err(e);
        }
    };
    swap_in(&work, target)?;
    Ok(hashes)
}

/// Podmienia `target` na `work`: stary katalog odkładany (`.old-…`) i usuwany dopiero po udanej
/// zamianie; błąd (np. sidecar uruchomiony — Windows blokuje pliki w użyciu) zostawia starą wersję.
fn swap_in(work: &Path, target: &Path) -> Result<(), Unsafe> {
    let name = target
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let backup = target.with_file_name(format!(".old-{name}"));
    if backup.exists() {
        let _ = std::fs::remove_dir_all(&backup);
    }
    let had_old = target.exists();
    if had_old && let Err(e) = std::fs::rename(target, &backup) {
        let _ = std::fs::remove_dir_all(work);
        return Err(bad(format!(
            "{}: {e} (zamknij program i spróbuj ponownie)",
            target.display()
        )));
    }
    if let Err(e) = std::fs::rename(work, target) {
        if had_old {
            let _ = std::fs::rename(&backup, target);
        }
        let _ = std::fs::remove_dir_all(work);
        return Err(bad(format!("{}: {e}", target.display())));
    }
    if had_old {
        let _ = std::fs::remove_dir_all(&backup);
    }
    Ok(())
}

fn trees_into(
    archives: &[PathBuf],
    work: &Path,
    strip: &str,
    require: &[String],
    limits: PackageLimits,
) -> Result<Hashes, Unsafe> {
    let mut hashes = Hashes::new();
    let mut seen = BTreeSet::new();
    for archive in archives {
        tree_into(archive, work, strip, limits, (&mut hashes, &mut seen))?;
    }
    for file in require {
        if !work.join(file).is_file() {
            return Err(bad(format!(
                "po rozpakowaniu brak „{file}” (inny układ archiwum — do potwierdzenia w katalogu)"
            )));
        }
    }
    Ok(hashes)
}

fn tree_into(
    archive: &Path,
    work: &Path,
    strip: &str,
    limits: PackageLimits,
    (hashes, seen): (&mut Hashes, &mut BTreeSet<String>),
) -> Result<(), Unsafe> {
    let mut reader = Reader::open(archive, limits)?;
    for i in 0..reader.archive.len() {
        let (name, dir, size) = reader.check(i)?;
        let Some(rel) = name.strip_prefix(strip) else {
            continue;
        };
        let rel = rel.trim_end_matches('/');
        if rel.is_empty() {
            continue;
        }
        validate_package_path(rel).map_err(bad)?;
        let path = work.join(rel);
        if dir {
            std::fs::create_dir_all(&path).map_err(bad)?;
            continue;
        }
        if !seen.insert(rel.to_lowercase()) {
            return Err(bad(format!("„{rel}” w więcej niż jednym archiwum")));
        }
        let sha = reader.copy(i, size, &path)?;
        hashes.insert(rel.to_owned(), sha);
    }
    Ok(())
}
