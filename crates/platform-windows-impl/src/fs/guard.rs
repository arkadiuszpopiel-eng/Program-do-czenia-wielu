//! Deny-lista ścieżek poświadczeń i kanoniczne rozwiązywanie ścieżek — ostatnia linia obrony
//! (docs/modules/platform-windows/SPEC.md, „Niezmienniki”; AGENTS.md: nigdy `~/.claude`, `~/.codex`).
//!
//! Każda ścieżka jest sprawdzana w kilku postaciach: surowej, po rozwinięciu `%ZMIENNYCH%`,
//! po normalizacji leksykalnej (`.`/`..`, prefiks `\\?\`, wielkość liter, końcowe kropki i spacje,
//! strumienie ADS `nazwa:strumień`) oraz kanonicznej (dowiązania, junctions i nazwy 8.3 rozwiązane
//! przez `std::fs::canonicalize`, które na Windows używa `GetFinalPathNameByHandleW`).

use std::path::{Component, Path, PathBuf};

use platform_contract::{PlatformError, is_credential_path};

/// Dodatkowe nazwy segmentów blokowane przez implementację (obrona w głąb ponad listę kontraktu).
pub const DEFAULT_EXTRA_DENY_NAMES: [&str; 12] = [
    ".aws",
    ".azure",
    ".docker",
    ".git-credentials",
    ".gnupg",
    ".kube",
    ".netrc",
    "_netrc",
    "cookies.sqlite",
    "key4.db",
    "Local State",
    "logins.json",
];

/// Dodatkowe prefiksy blokowane przez implementację (klucze DPAPI, sejf poświadczeń).
pub const DEFAULT_EXTRA_DENY_PREFIXES: [&str; 4] = [
    "%APPDATA%\\Microsoft\\Protect",
    "%APPDATA%\\Microsoft\\Crypto",
    "%APPDATA%\\Microsoft\\Vault",
    "%LOCALAPPDATA%\\Microsoft\\Vault",
];

/// Czy operacja dotyczy celu dowiązania (`Final`) czy samego wpisu (`NoFinal`: usuwanie, przenoszenie).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Follow {
    /// Ostatni segment jest rozwiązywany (odczyt, zapis, źródło kopii).
    Final,
    /// Ostatni segment zostaje (operacja na samym wpisie).
    NoFinal,
}

/// Polityka deny-listy: lista kontraktu + nazwy i prefiksy z konfiguracji.
#[derive(Debug, Clone, Default)]
pub(crate) struct DenyPolicy {
    names: Vec<String>,
    prefixes: Vec<Vec<String>>,
}

impl DenyPolicy {
    /// Buduje politykę; prefiksy są rozwijane (`%APPDATA%`) i normalizowane.
    pub(crate) fn new(names: &[String], prefixes: &[PathBuf]) -> Self {
        Self {
            names: names.iter().map(|n| segment_key(n)).collect(),
            prefixes: prefixes
                .iter()
                .map(|p| keys_of(&lexical_normalize(&expand_env(p))))
                .filter(|keys| !keys.is_empty())
                .collect(),
        }
    }

    /// Czy ścieżka (w dowolnej postaci) trafia na deny-listę.
    pub(crate) fn is_denied(&self, path: &Path) -> bool {
        if is_credential_path(path) {
            return true;
        }
        let keys = unc_admin_share_to_drive(keys_of(path));
        if is_credential_path(&keys.iter().collect::<PathBuf>()) {
            return true;
        }
        if keys.iter().any(|k| self.names.contains(k)) {
            return true;
        }
        self.prefixes
            .iter()
            .any(|prefix| keys.len() >= prefix.len() && keys[..prefix.len()] == prefix[..])
    }

    /// Rozwiązuje ścieżkę do postaci używanej w operacji i sprawdza wszystkie jej postaci.
    /// Błąd `Denylisted` zawiera ścieżkę podaną przez wywołującego.
    pub(crate) fn resolve(&self, path: &Path, follow: Follow) -> Result<PathBuf, PlatformError> {
        if path.as_os_str().is_empty() {
            return Err(PlatformError::InvalidPath(path.to_path_buf()));
        }
        let expanded = expand_env(path);
        if !expanded.is_absolute() {
            return Err(PlatformError::InvalidPath(path.to_path_buf()));
        }
        let lexical = lexical_normalize(&expanded);
        let deny = || PlatformError::Denylisted(path.to_path_buf());
        for form in [path, expanded.as_path(), lexical.as_path()] {
            if self.is_denied(form) {
                return Err(deny());
            }
        }
        let Some(name) = lexical.file_name().map(ToOwned::to_owned) else {
            // Korzeń woluminu: nie ma czego rozwiązywać.
            return Ok(lexical);
        };
        let parent = lexical.parent().unwrap_or(&lexical);
        let op_path = canonical_prefix(parent, path)?.join(name);
        if self.is_denied(&op_path) {
            return Err(deny());
        }
        if (follow == Follow::Final || op_path.is_symlink())
            && let Ok(full) = std::fs::canonicalize(&op_path)
            && self.is_denied(&full)
        {
            return Err(deny());
        }
        Ok(op_path)
    }
}

/// Kanoniczna postać najdłuższego istniejącego przodka + pozostałe (nieistniejące) segmenty.
fn canonical_prefix(dir: &Path, original: &Path) -> Result<PathBuf, PlatformError> {
    let mut existing = dir.to_path_buf();
    let mut rest: Vec<std::ffi::OsString> = Vec::new();
    loop {
        if std::fs::symlink_metadata(&existing).is_ok() {
            let canonical = std::fs::canonicalize(&existing).map_err(|_| {
                PlatformError::PermissionDenied(format!(
                    "{}: nie można kanonicznie rozwiązać ścieżki (zerwane dowiązanie?)",
                    original.display()
                ))
            })?;
            return Ok(rest.iter().rev().fold(canonical, |acc, seg| acc.join(seg)));
        }
        match (existing.file_name(), existing.parent()) {
            (Some(seg), Some(parent)) => {
                rest.push(seg.to_owned());
                existing = parent.to_path_buf();
            }
            _ => return Ok(dir.to_path_buf()),
        }
    }
}

/// Rozwija `%ZMIENNE%` środowiskowe (tylko zdefiniowane; ścieżki nie-Unicode bez zmian).
pub(crate) fn expand_env(path: &Path) -> PathBuf {
    let Some(text) = path.to_str() else {
        return path.to_path_buf();
    };
    if !text.contains('%') {
        return path.to_path_buf();
    }
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find('%') {
        out.push_str(&rest[..start]);
        let after = &rest[start + 1..];
        match after.find('%') {
            Some(end) if end > 0 => {
                let name = &after[..end];
                match std::env::var(name) {
                    Ok(value) => out.push_str(&value),
                    Err(_) => {
                        out.push('%');
                        out.push_str(name);
                        out.push('%');
                    }
                }
                rest = &after[end + 1..];
            }
            _ => {
                out.push('%');
                rest = after;
            }
        }
    }
    out.push_str(rest);
    PathBuf::from(out)
}

/// Normalizacja leksykalna: usuwa `.`, rozwiązuje `..` (bez wychodzenia ponad korzeń).
pub(crate) fn lexical_normalize(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                if matches!(out.components().next_back(), Some(Component::Normal(_))) {
                    out.pop();
                }
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
}

/// Klucz segmentu do porównań: małe litery, bez strumienia ADS, bez końcowych kropek i spacji.
pub(crate) fn segment_key(segment: &str) -> String {
    let lower = segment.to_lowercase();
    let is_drive = lower.len() == 2
        && lower.ends_with(':')
        && lower
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_alphabetic());
    let base = match lower.find(':') {
        Some(idx) if !is_drive => &lower[..idx],
        _ => lower.as_str(),
    };
    base.trim_end_matches(['.', ' ']).to_owned()
}

/// Klucze segmentów ścieżki (oba separatory, bez prefiksów `\\?\`, `\\?\UNC\`, `\??\`, `\\.\`).
pub(crate) fn keys_of(path: &Path) -> Vec<String> {
    let text = path.to_string_lossy().replace('/', "\\");
    let stripped = ["\\\\?\\UNC\\", "\\??\\UNC\\"]
        .iter()
        .find_map(|p| text.strip_prefix(p).map(|rest| format!("\\\\{rest}")))
        .or_else(|| {
            ["\\\\?\\", "\\??\\", "\\\\.\\"]
                .iter()
                .find_map(|p| text.strip_prefix(p).map(str::to_owned))
        })
        .unwrap_or(text);
    stripped
        .split('\\')
        .map(segment_key)
        .filter(|k| !k.is_empty())
        .collect()
}

/// `\\host\c$\reszta` → `c:\reszta` (udział administracyjny wskazujący lokalny dysk).
fn unc_admin_share_to_drive(keys: Vec<String>) -> Vec<String> {
    let is_admin_share = keys.get(1).is_some_and(|share| {
        share.len() == 2
            && share.ends_with('$')
            && share
                .chars()
                .next()
                .is_some_and(|c| c.is_ascii_alphabetic())
    });
    if !is_admin_share {
        return keys;
    }
    let mut out = Vec::with_capacity(keys.len() - 1);
    out.push(format!("{}:", &keys[1][..1]));
    out.extend(keys.into_iter().skip(2));
    out
}

#[cfg(test)]
#[path = "guard_tests.rs"]
mod tests;
