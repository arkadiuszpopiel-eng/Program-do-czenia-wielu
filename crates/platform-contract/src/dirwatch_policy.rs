//! Polityka obserwacji katalogów: deny-lista (segmenty i prefiksy, postać surowa i kanoniczna,
//! wielkość liter, końcowe kropki/spacje, strumienie ADS), limity, filtr nazw (pliki tymczasowe,
//! wzorce `*`/`?`).

use std::path::{Component, Path, PathBuf};
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::dirwatch::{
    BASELINE_DENY_SEGMENTS, DEFAULT_DEBOUNCE_MS, DEFAULT_MAX_DELAY_MS, DEFAULT_MAX_WATCH_ENTRIES,
    DEFAULT_MAX_WATCHES, MAX_PATTERN_LEN, MAX_WATCH_PATTERNS, WatchSpec,
};
use crate::error::PlatformError;
use crate::fs::is_credential_path;

/// Polityka obserwacji (deny-lista, limity, debounce).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WatchPolicy {
    /// Segmenty zabronione w dowolnym miejscu ścieżki (bez rozróżniania wielkości liter).
    pub deny_segments: Vec<String>,
    /// Prefiksy zabronione (ścieżki bezwzględne po rozwinięciu zmiennych środowiska).
    pub deny_prefixes: Vec<PathBuf>,
    /// Najwięcej obserwacji naraz.
    pub max_watches: usize,
    /// Najwięcej plików pamiętanych na obserwację.
    pub max_entries: usize,
    /// Cisza przed zgłoszeniem (ms).
    pub debounce_ms: u64,
    /// Najdłuższe opóźnienie zgłoszenia (ms).
    pub max_delay_ms: u64,
}

impl Default for WatchPolicy {
    fn default() -> Self {
        Self::baseline()
    }
}

/// Postać porównywana: małe litery, `/`, bez `\\?\`, bez końcowych kropek/spacji i strumieni ADS.
fn norm_segment(s: &str) -> String {
    let s = s.split(':').next().unwrap_or(s);
    s.trim_end_matches(['.', ' ']).to_lowercase()
}

fn norm_path(p: &Path) -> String {
    let s = p.to_string_lossy().replace('\\', "/");
    let s = s
        .strip_prefix("//?/UNC/")
        .map(|r| format!("//{r}"))
        .unwrap_or_else(|| s.strip_prefix("//?/").unwrap_or(&s).to_owned());
    s.trim_end_matches('/').to_lowercase()
}

impl WatchPolicy {
    /// Lista bazowa i domyślne limity.
    pub fn baseline() -> Self {
        Self {
            deny_segments: BASELINE_DENY_SEGMENTS.iter().map(|s| (*s).into()).collect(),
            deny_prefixes: Vec::new(),
            max_watches: DEFAULT_MAX_WATCHES,
            max_entries: DEFAULT_MAX_WATCH_ENTRIES,
            debounce_ms: DEFAULT_DEBOUNCE_MS,
            max_delay_ms: DEFAULT_MAX_DELAY_MS,
        }
    }

    /// Dokłada deny-listę Jądra (`compliance::DenyLists`: segmenty i rozwinięte prefiksy).
    #[must_use]
    pub fn with_denylist<S, P>(mut self, segments: S, prefixes: P) -> Self
    where
        S: IntoIterator<Item = String>,
        P: IntoIterator<Item = PathBuf>,
    {
        for s in segments {
            if !self
                .deny_segments
                .iter()
                .any(|d| d.eq_ignore_ascii_case(&s))
            {
                self.deny_segments.push(s);
            }
        }
        self.deny_prefixes.extend(prefixes);
        self
    }

    /// Debounce jako `Duration`.
    pub fn debounce(&self) -> Duration {
        Duration::from_millis(self.debounce_ms)
    }

    /// Czy ścieżka (albo jej przodek) jest na deny-liście.
    pub fn is_denied(&self, path: &Path) -> bool {
        if is_credential_path(path) {
            return true;
        }
        let segment_denied = path.components().any(|c| match c {
            Component::Normal(seg) => {
                let seg = norm_segment(&seg.to_string_lossy());
                self.deny_segments.iter().any(|d| norm_segment(d) == seg)
            }
            _ => false,
        });
        if segment_denied {
            return true;
        }
        let p = norm_path(path);
        self.deny_prefixes.iter().any(|d| {
            let d = norm_path(d);
            !d.is_empty() && (p == d || p.starts_with(&format!("{d}/")))
        })
    }

    /// Sprawdza obserwację: ścieżka bezwzględna, nie korzeń wolumenu z podkatalogami, katalog
    /// (surowy i kanoniczny — po rozwiązaniu dowiązań/junctions) poza deny-listą, poprawne wzorce.
    pub fn check_spec(
        &self,
        spec: &WatchSpec,
        canonical: Option<&Path>,
    ) -> Result<(), PlatformError> {
        if !spec.dir.is_absolute() {
            return Err(PlatformError::InvalidPath(spec.dir.clone()));
        }
        if spec.recursive && spec.dir.parent().is_none() {
            return Err(PlatformError::PermissionDenied(
                "obserwacja całego wolumenu z podkatalogami jest niedozwolona".into(),
            ));
        }
        for dir in std::iter::once(spec.dir.as_path()).chain(canonical) {
            if self.is_denied(dir) {
                return Err(PlatformError::Denylisted(dir.to_path_buf()));
            }
        }
        if spec.patterns.len() > MAX_WATCH_PATTERNS
            || spec
                .patterns
                .iter()
                .any(|p| p.is_empty() || p.chars().count() > MAX_PATTERN_LEN)
        {
            return Err(PlatformError::Unsupported(format!(
                "wzorce nazw: najwyżej {MAX_WATCH_PATTERNS}, każdy 1–{MAX_PATTERN_LEN} znaków"
            )));
        }
        Ok(())
    }
}

/// Plik tymczasowy: pobieranie w toku, blokady i kopie robocze edytorów, pliki systemowe powłoki.
pub fn is_temp_name(name: &str) -> bool {
    const SUFFIXES: [&str; 10] = [
        ".tmp",
        ".temp",
        ".part",
        ".partial",
        ".crdownload",
        ".download",
        ".opdownload",
        ".swp",
        ".swx",
        ".!ut",
    ];
    const NAMES: [&str; 4] = ["4913", "thumbs.db", "desktop.ini", ".ds_store"];
    let lower = name.to_lowercase();
    lower.starts_with("~$")
        || lower.starts_with(".~lock.")
        || lower.starts_with(".#")
        || lower.ends_with('~')
        || NAMES.contains(&lower.as_str())
        || SUFFIXES.iter().any(|s| lower.ends_with(s))
}

/// Dopasowanie wzorca `*`/`?` bez rozróżniania wielkości liter.
pub fn glob_match(pattern: &str, name: &str) -> bool {
    let p: Vec<char> = pattern.to_lowercase().chars().collect();
    let n: Vec<char> = name.to_lowercase().chars().collect();
    let (mut pi, mut ni) = (0, 0);
    let mut star: Option<(usize, usize)> = None;
    while ni < n.len() {
        match p.get(pi) {
            Some('*') => {
                star = Some((pi, ni));
                pi += 1;
            }
            Some(&c) if c == '?' || c == n[ni] => {
                pi += 1;
                ni += 1;
            }
            _ => match star {
                Some((sp, sn)) => {
                    pi = sp + 1;
                    ni = sn + 1;
                    star = Some((sp, sn + 1));
                }
                None => return false,
            },
        }
    }
    p[pi..].iter().all(|&c| c == '*')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn temp_names_and_globs() {
        for t in [
            "raport.pdf.crdownload",
            "~$Umowa.docx",
            "x.TMP",
            ".~lock.a.odt#",
            "plik~",
            "Thumbs.db",
            "film.part",
        ] {
            assert!(is_temp_name(t), "{t}");
        }
        assert!(!is_temp_name("faktura.pdf"));
        assert!(glob_match("*.PDF", "faktura.pdf"));
        assert!(glob_match("f?ktura*", "Faktura-2026.pdf"));
        assert!(!glob_match("*.pdf", "x.pdfx"));
        assert!(glob_match("a*b*c", "aXbYc"));
        assert!(!glob_match("a*b*c", "aXbY"));
    }
}
