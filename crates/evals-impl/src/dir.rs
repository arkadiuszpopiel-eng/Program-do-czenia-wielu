//! Katalog zestawów z dysku: odkrywanie manifestów, bezpieczny odczyt plików, adaptery formatów.
//! Katalogi zapieczętowane (`holdout/`, `corpus/`) nie są nigdy czytane przez katalog publiczny.

use std::collections::BTreeMap;
use std::fs;
use std::io::ErrorKind;
use std::path::{Component, Path, PathBuf};

use evals_contract::{
    EvalCase, EvalError, IntegrityReport, LegacyManifest, Split, SuiteCatalog, SuiteId, SuiteInfo,
    SuiteManifest, parse_cases, validate_rel_path, verify_files,
};

/// Katalogi zapieczętowane względem korzenia (holdout i korpus właściciela).
pub const SEALED_DIRS: [&str; 2] = ["holdout", "corpus"];
/// Katalogi pomijane przy odkrywaniu.
const SKIPPED_DIRS: [&str; 3] = ["target", "node_modules", ".git"];
/// Maksymalna głębokość odkrywania.
const MAX_DEPTH: usize = 8;
/// Maksymalny rozmiar pliku zestawu (64 MiB).
const MAX_FILE_BYTES: u64 = 64 * 1024 * 1024;

fn io(e: &std::io::Error, path: &Path) -> EvalError {
    EvalError::Io(format!("{}: {e}", path.display()))
}

/// Bezpieczny odczyt pliku `rel` spod `root` (ścieżka po kanonizacji musi leżeć w `root`
/// i — gdy `sealed` niepuste — poza katalogami zapieczętowanymi). `Ok(None)` = brak pliku.
pub(crate) fn safe_read(
    root: &Path,
    rel: &str,
    sealed: &[PathBuf],
) -> Result<Option<Vec<u8>>, EvalError> {
    validate_rel_path(rel)?;
    if !sealed.is_empty() && is_sealed_rel(rel) {
        return Err(EvalError::HoldoutSealed);
    }
    let path = root.join(rel);
    let canonical = match fs::canonicalize(&path) {
        Ok(c) => c,
        Err(e) if e.kind() == ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(io(&e, &path)),
    };
    if !canonical.starts_with(root) {
        return Err(EvalError::UnsafePath(rel.to_owned()));
    }
    if sealed.iter().any(|s| canonical.starts_with(s)) {
        return Err(EvalError::HoldoutSealed);
    }
    let meta = fs::metadata(&canonical).map_err(|e| io(&e, &canonical))?;
    if !meta.is_file() || meta.len() > MAX_FILE_BYTES {
        return Err(EvalError::Io(format!(
            "`{rel}` nie jest plikiem albo jest za duży"
        )));
    }
    fs::read(&canonical)
        .map(Some)
        .map_err(|e| io(&e, &canonical))
}

/// Czy ścieżka względna wskazuje katalog zapieczętowany (bez rozróżniania wielkości liter).
fn is_sealed_rel(rel: &str) -> bool {
    let first = rel
        .split('/')
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase();
    SEALED_DIRS.contains(&first.as_str())
}

fn rel_string(root: &Path, path: &Path) -> Option<String> {
    let rel = path.strip_prefix(root).ok()?;
    let parts: Option<Vec<&str>> = rel
        .components()
        .map(|c| match c {
            Component::Normal(s) => s.to_str(),
            _ => None,
        })
        .collect();
    Some(parts?.join("/"))
}

/// Pliki manifestów pod `dir` (bez dowiązań, bez katalogów pomijanych i — opcjonalnie — zapieczętowanych).
pub(crate) fn discover(root: &Path, skip_sealed: bool) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let mut stack = vec![(root.to_path_buf(), 0usize)];
    while let Some((dir, depth)) = stack.pop() {
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().to_ascii_lowercase();
            let Ok(meta) = fs::symlink_metadata(&path) else {
                continue;
            };
            if meta.is_dir() {
                let sealed = depth == 0 && SEALED_DIRS.contains(&name.as_str());
                let skip = name.starts_with('.') || SKIPPED_DIRS.contains(&name.as_str());
                if depth < MAX_DEPTH && !skip && !(skip_sealed && sealed) {
                    stack.push((path, depth + 1));
                }
            } else if meta.is_file() && (name == "manifest.json" || name.ends_with(".suite.json")) {
                found.push(path);
            }
        }
    }
    found.sort();
    found
}

/// Wczytuje manifest (format natywny albo stary format F5 — wtedy ścieżki względem katalogu).
pub(crate) fn load_manifest(root: &Path, path: &Path) -> Result<SuiteManifest, String> {
    let rel = rel_string(root, path).ok_or("ścieżka spoza korzenia")?;
    let bytes = fs::read(path).map_err(|e| e.to_string())?;
    let value: serde_json::Value = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
    if value.get("schema").is_some() {
        return serde_json::from_value(value).map_err(|e| e.to_string());
    }
    let legacy: LegacyManifest = serde_json::from_value(value).map_err(|e| e.to_string())?;
    let dir = rel
        .rsplit_once('/')
        .map(|(d, _)| d)
        .ok_or("stary manifest w korzeniu")?;
    legacy.into_suite(dir).map_err(|e| e.to_string())
}

#[derive(Debug, Clone)]
struct Loaded {
    manifest: SuiteManifest,
    manifest_path: String,
}

/// Katalog zestawów publicznych z korzenia `evals/`.
#[derive(Debug)]
pub struct DirCatalog {
    root: PathBuf,
    sealed: Vec<PathBuf>,
    suites: BTreeMap<SuiteId, Loaded>,
    problems: Vec<String>,
}

impl DirCatalog {
    /// Otwiera katalog; niepoprawne manifesty trafiają do [`DirCatalog::problems`] i są pomijane.
    pub fn open(root: impl AsRef<Path>) -> Result<Self, EvalError> {
        let root = fs::canonicalize(root.as_ref()).map_err(|e| io(&e, root.as_ref()))?;
        let mut sealed = SEALED_DIRS
            .iter()
            .filter_map(|d| fs::canonicalize(root.join(d)).ok())
            .collect::<Vec<_>>();
        // Nieistniejący katalog zapieczętowany: pilnuje go sprawdzenie pierwszego segmentu ścieżki.
        sealed.push(root.join(SEALED_DIRS[0]));
        let mut catalog = Self {
            root: root.clone(),
            sealed,
            suites: BTreeMap::new(),
            problems: Vec::new(),
        };
        for path in discover(&root, true) {
            let rel = rel_string(&root, &path).unwrap_or_default();
            match load_manifest(&root, &path).and_then(|m| {
                m.validate_public().map_err(|e| e.to_string())?;
                if m.files.keys().any(|p| is_sealed_rel(p)) {
                    return Err(EvalError::HoldoutSealed.to_string());
                }
                Ok(m)
            }) {
                Ok(m) if catalog.suites.contains_key(&m.suite) => {
                    catalog
                        .problems
                        .push(format!("{rel}: powtórzony zestaw `{}`", m.suite));
                }
                Ok(manifest) => {
                    catalog.suites.insert(
                        manifest.suite.clone(),
                        Loaded {
                            manifest,
                            manifest_path: rel,
                        },
                    );
                }
                Err(e) => catalog.problems.push(format!("{rel}: {e}")),
            }
        }
        Ok(catalog)
    }

    /// Kanoniczny korzeń.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Problemy wykryte przy otwarciu (manifest niepoprawny, powtórzony, z holdoutem).
    pub fn problems(&self) -> &[String] {
        &self.problems
    }

    fn get(&self, suite: &SuiteId) -> Result<&Loaded, EvalError> {
        self.suites
            .get(suite)
            .ok_or_else(|| EvalError::UnknownSuite(suite.to_string()))
    }

    fn read(&self, rel: &str) -> Result<Option<Vec<u8>>, EvalError> {
        safe_read(&self.root, rel, &self.sealed)
    }

    fn load_cases(
        &self,
        manifest: &SuiteManifest,
        split: Split,
    ) -> Result<Vec<EvalCase>, EvalError> {
        let mut out = Vec::new();
        for source in &manifest.cases {
            let bytes = self
                .read(&source.path)?
                .ok_or_else(|| EvalError::Io(format!("brak pliku `{}`", source.path)))?;
            out.extend(
                parse_cases(source, &bytes)?
                    .into_iter()
                    .filter(|c| c.split == split),
            );
        }
        Ok(out)
    }
}

impl SuiteCatalog for DirCatalog {
    fn suites(&self) -> Vec<SuiteInfo> {
        self.suites
            .values()
            .map(|l| SuiteInfo {
                suite: l.manifest.suite.clone(),
                wave: l.manifest.wave.clone(),
                version: l.manifest.version,
                status: l.manifest.status,
                digest: l.manifest.digest(),
                manifest_path: l.manifest_path.clone(),
                case_counts: [Split::Dev, Split::Test]
                    .into_iter()
                    .filter_map(|s| {
                        let n = self.load_cases(&l.manifest, s).map_or(0, |c| c.len());
                        (n > 0).then_some((s, n))
                    })
                    .collect(),
            })
            .collect()
    }

    fn manifest(&self, suite: &SuiteId) -> Result<SuiteManifest, EvalError> {
        Ok(self.get(suite)?.manifest.clone())
    }

    fn verify(&self, suite: &SuiteId) -> Result<IntegrityReport, EvalError> {
        let loaded = self.get(suite)?;
        Ok(verify_files(&loaded.manifest, |p| {
            self.read(p).ok().flatten()
        }))
    }

    fn cases(&self, suite: &SuiteId, split: Split) -> Result<Vec<EvalCase>, EvalError> {
        if split == Split::Holdout {
            return Err(EvalError::HoldoutSealed);
        }
        self.verify(suite)?.enforce(false)?;
        self.load_cases(&self.get(suite)?.manifest, split)
    }
}
