//! Stan menedżera na dysku: katalog roboczy pobrań `%LOCALAPPDATA%\Alfa\downloads\<id>\` (pliki
//! `.part` do wznowienia, `pending.json` = pobrane bez przypiętego hasha, czeka na zgodę TOFU) i
//! rekordy instalacji `%LOCALAPPDATA%\Alfa\state\models\<id>.json` (SHA-256 pobranych i wynikowych
//! plików, czy hash zaakceptowano przy pierwszym użyciu). Bez sekretów i bez telemetrii.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use app_api::AppPaths;
use serde::{Deserialize, Serialize};

use crate::catalog::ItemSpec;
use crate::fetch::part_path;

/// Pliki pobrane bez przypiętego hasha — czekają na zgodę (nazwa → SHA-256 policzony w locie).
pub const PENDING_FILE: &str = "pending.json";

/// SHA-256 plików: nazwa → hash.
pub type Hashes = BTreeMap<String, String>;

/// Rekord instalacji.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Receipt {
    /// Pozycja.
    pub id: String,
    /// Hash zaakceptowany przy pierwszym użyciu (zamiast przypiętego w katalogu).
    pub trusted: bool,
    /// Pobrane pliki (nazwa → SHA-256).
    pub downloads: Hashes,
    /// Pliki wynikowe względem katalogu pozycji (ścieżka → SHA-256) — weryfikacja.
    pub files: Hashes,
    /// Wynik ostatniej weryfikacji, jeśli wykryła niezgodność.
    #[serde(default)]
    pub corrupt: Option<String>,
}

/// Oczekujące na zgodę.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Pending {
    /// Pobrane pliki (nazwa → SHA-256).
    pub hashes: Hashes,
}

/// Czy identyfikator nadaje się na nazwę pliku (`[A-Za-z0-9._-]`, bez kropki na początku).
pub fn safe_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 128
        && !id.starts_with('.')
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
}

/// Dostęp do stanu na dysku.
#[derive(Debug, Clone)]
pub struct Store {
    paths: AppPaths,
}

fn write_json<T: Serialize>(path: &Path, value: &T) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp = part_path(path);
    let text = serde_json::to_string_pretty(value).map_err(std::io::Error::other)?;
    std::fs::write(&tmp, text)?;
    std::fs::rename(&tmp, path)
}

fn read_json<T: for<'de> Deserialize<'de>>(path: &Path) -> Option<T> {
    let text = std::fs::read_to_string(path).ok()?;
    match serde_json::from_str(&text) {
        Ok(v) => Some(v),
        Err(e) => {
            tracing::warn!(path = %path.display(), error = %e, "nieczytelny stan menedżera modeli");
            None
        }
    }
}

impl Store {
    /// Stan pod katalogami aplikacji.
    pub fn new(paths: AppPaths) -> Self {
        Self { paths }
    }

    /// Katalogi aplikacji.
    pub fn paths(&self) -> &AppPaths {
        &self.paths
    }

    /// Katalog roboczy pobrań pozycji.
    pub fn staging(&self, id: &str) -> PathBuf {
        self.paths.local.join("downloads").join(id)
    }

    fn receipt_path(&self, id: &str) -> PathBuf {
        self.paths.state().join("models").join(format!("{id}.json"))
    }

    /// Rekord instalacji.
    pub fn receipt(&self, id: &str) -> Option<Receipt> {
        read_json(&self.receipt_path(id))
    }

    /// Zapis rekordu (atomowo).
    pub fn save_receipt(&self, receipt: &Receipt) -> std::io::Result<()> {
        write_json(&self.receipt_path(&receipt.id), receipt)
    }

    /// Usuwa rekord.
    pub fn drop_receipt(&self, id: &str) -> std::io::Result<()> {
        match std::fs::remove_file(self.receipt_path(id)) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e),
            _ => Ok(()),
        }
    }

    /// Oczekujące na zgodę TOFU.
    pub fn pending(&self, id: &str) -> Option<Pending> {
        read_json(&self.staging(id).join(PENDING_FILE))
    }

    /// Zapis oczekujących.
    pub fn save_pending(&self, id: &str, pending: &Pending) -> std::io::Result<()> {
        write_json(&self.staging(id).join(PENDING_FILE), pending)
    }

    /// Usuwa katalog roboczy pozycji (pliki częściowe, pobrane, `pending.json`).
    pub fn clear_staging(&self, id: &str) -> std::io::Result<()> {
        match std::fs::remove_dir_all(self.staging(id)) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e),
            _ => Ok(()),
        }
    }

    /// Plik częściowy do wznowienia: (plik, pobrane bajty).
    pub fn partial(&self, spec: &ItemSpec) -> Option<(String, u64)> {
        let staging = self.staging(&spec.id);
        spec.files.iter().find_map(|f| {
            let part = part_path(&staging.join(&f.name));
            std::fs::metadata(part)
                .ok()
                .map(|m| (f.name.clone(), m.len()))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_safe_file_names() {
        assert!(safe_id("multilingual-e5-small"));
        assert!(safe_id("whisper-large-v3-turbo-q5_0"));
        for bad in ["", ".hidden", "../x", "a/b", "a\\b", "a:b", "ę"] {
            assert!(!safe_id(bad), "{bad}");
        }
    }

    #[test]
    fn receipts_and_pending_roundtrip() {
        let dir = std::env::temp_dir().join(format!("alfa-models-store-{}", std::process::id()));
        let store = Store::new(AppPaths::under(&dir));
        assert!(store.receipt("x").is_none());
        let receipt = Receipt {
            id: "x".into(),
            trusted: true,
            downloads: Hashes::from([("a.bin".into(), "0".repeat(64))]),
            files: Hashes::from([("a.bin".into(), "0".repeat(64))]),
            corrupt: None,
        };
        store.save_receipt(&receipt).unwrap();
        assert_eq!(store.receipt("x"), Some(receipt));
        store.save_pending("x", &Pending::default()).unwrap();
        assert_eq!(store.pending("x"), Some(Pending::default()));
        store.clear_staging("x").unwrap();
        store.drop_receipt("x").unwrap();
        assert!(store.pending("x").is_none() && store.receipt("x").is_none());
        let _ = std::fs::remove_dir_all(dir);
    }
}
