//! [`DocumentStore`] na katalogu (konfiguracja, agentki, pamięć…): nazwy wg reguły ścieżek
//! paczki, filtr rozszerzeń (np. konfiguracja przyjmuje tylko `*.toml`), zapis atomowy
//! (plik tymczasowy + `rename`), żadnego zapisu poza korzeniem (także przez dowiązania).

use std::path::{Component, Path, PathBuf};

use transfer_contract::{DocumentStore, TransferError, validate_entry_path};

use crate::tempfile::{TEMP_SUFFIX, TempFile};

/// Które pliki należą do magazynu.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DirFilter {
    /// Podkatalogi (rekurencyjnie).
    pub recursive: bool,
    /// Dozwolone rozszerzenia (małe litery, bez kropki); puste = wszystkie.
    pub extensions: Vec<String>,
    /// Nazwy pomijane (np. `history.ndjson` w katalogu konfiguracji).
    pub exclude: Vec<String>,
}

impl DirFilter {
    /// Tylko pliki o podanych rozszerzeniach, bez podkatalogów.
    pub fn flat(extensions: &[&str]) -> Self {
        Self {
            recursive: false,
            extensions: extensions.iter().map(|e| (*e).to_owned()).collect(),
            exclude: Vec::new(),
        }
    }

    /// Wszystkie pliki, rekurencyjnie.
    pub fn tree() -> Self {
        Self {
            recursive: true,
            ..Self::default()
        }
    }

    fn accepts(&self, name: &str) -> bool {
        if (!self.recursive && name.contains('/')) || self.exclude.iter().any(|x| x == name) {
            return false;
        }
        if name.ends_with(TEMP_SUFFIX) {
            return false;
        }
        self.extensions.is_empty()
            || name
                .rsplit_once('.')
                .is_some_and(|(_, ext)| self.extensions.iter().any(|e| e.eq_ignore_ascii_case(ext)))
    }
}

/// Magazyn dokumentów w katalogu.
#[derive(Debug, Clone)]
pub struct DirDocumentStore {
    root: PathBuf,
    filter: DirFilter,
}

impl DirDocumentStore {
    /// Magazyn w `root` (katalog tworzony przy pierwszym zapisie).
    pub fn new(root: impl Into<PathBuf>, filter: DirFilter) -> Self {
        Self {
            root: root.into(),
            filter,
        }
    }

    /// Korzeń.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Ścieżka dokumentu (po walidacji nazwy i filtra).
    fn resolve(&self, name: &str) -> Result<PathBuf, TransferError> {
        validate_entry_path(name).map_err(|reason| TransferError::UnsafePath {
            path: name.to_owned(),
            reason,
        })?;
        if !self.filter.accepts(name) {
            return Err(TransferError::invalid(
                name,
                "nazwa spoza magazynu (filtr rozszerzeń/katalogów)",
            ));
        }
        let path = name.split('/').fold(self.root.clone(), |p, s| p.join(s));
        let escapes = path
            .strip_prefix(&self.root)
            .map(|rel| rel.components().any(|c| !matches!(c, Component::Normal(_))))
            .unwrap_or(true);
        if escapes {
            return Err(TransferError::invalid(name, "ścieżka poza magazynem"));
        }
        Ok(path)
    }

    /// Katalog nadrzędny istnieje i (po rozwinięciu dowiązań) leży w korzeniu.
    fn ensure_parent(&self, path: &Path) -> Result<(), TransferError> {
        let parent = path.parent().unwrap_or(&self.root);
        std::fs::create_dir_all(parent)?;
        let root = std::fs::canonicalize(&self.root)?;
        if !std::fs::canonicalize(parent)?.starts_with(&root) {
            return Err(TransferError::invalid(
                path.display().to_string(),
                "dowiązanie poza magazyn",
            ));
        }
        Ok(())
    }

    fn walk(&self, dir: &Path, prefix: &str, out: &mut Vec<String>) -> Result<(), TransferError> {
        let entries = match std::fs::read_dir(dir) {
            Ok(e) => e,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(e) => return Err(e.into()),
        };
        for entry in entries.flatten() {
            let Ok(kind) = entry.file_type() else {
                continue;
            };
            let Some(file) = entry.file_name().to_str().map(str::to_owned) else {
                continue;
            };
            let name = if prefix.is_empty() {
                file
            } else {
                format!("{prefix}/{file}")
            };
            if kind.is_dir() && self.filter.recursive {
                self.walk(&entry.path(), &name, out)?;
            } else if kind.is_file()
                && validate_entry_path(&name).is_ok()
                && self.filter.accepts(&name)
            {
                out.push(name);
            }
        }
        Ok(())
    }
}

impl DocumentStore for DirDocumentStore {
    fn list(&self) -> Result<Vec<String>, TransferError> {
        let mut out = Vec::new();
        self.walk(&self.root, "", &mut out)?;
        out.sort();
        Ok(out)
    }

    fn read(&self, name: &str) -> Result<Option<Vec<u8>>, TransferError> {
        let path = self.resolve(name)?;
        match std::fs::read(&path) {
            Ok(bytes) => Ok(Some(bytes)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    fn write(&self, name: &str, bytes: &[u8]) -> Result<(), TransferError> {
        let path = self.resolve(name)?;
        self.ensure_parent(&path)?;
        let dir = path.parent().unwrap_or(&self.root);
        let tmp = TempFile::new(dir, &path)?;
        {
            use std::io::Write;
            let mut file = tmp.create()?;
            file.write_all(bytes)?;
            file.sync_all()?;
        }
        tmp.persist(&path)?;
        Ok(())
    }

    fn remove(&self, name: &str) -> Result<bool, TransferError> {
        let path = self.resolve(name)?;
        match std::fs::remove_file(&path) {
            Ok(()) => Ok(true),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
            Err(e) => Err(e.into()),
        }
    }
}
