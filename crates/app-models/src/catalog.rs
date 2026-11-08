//! Katalog pozycji menedżera: co pobrać (adres, rozmiar, przypięty SHA-256), dokąd zainstalować
//! (korzeń `models`/`sidecars` + podkatalog) i jak (pliki, GGUF z zapisem hasha dla `providers-local`,
//! embedder z `embed.json`, manifest mówcy, wybrane wpisy archiwum, całe drzewo archiwum sidecara,
//! instalacja ręczna). Katalog produkcyjny: [`builtin`]; testy budują własne pozycje.

use std::path::{Path, PathBuf};

use app_api::AppPaths;
use app_api::dto::{LocalizedText, ModelItemKind};
use lib_embed::EmbedManifest;

pub mod target;

pub use target::Target;

const MIB: u64 = 1024 * 1024;

/// Korzeń katalogu docelowego.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Root {
    /// `%LOCALAPPDATA%\Alfa\models`.
    Models,
    /// `%LOCALAPPDATA%\Alfa\sidecars`.
    Sidecars,
}

/// Plik do pobrania.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileSpec {
    /// Nazwa (ścieżka względna w katalogu docelowym albo nazwa archiwum).
    pub name: String,
    /// Adres (`https://`; testy — `http://127.0.0.1`).
    pub url: String,
    /// Przybliżony rozmiar (B) — UI i limit pobierania.
    pub size: u64,
    /// Przypięty SHA-256 (hex); `None` — zgoda TOFU w UI.
    pub sha256: Option<String>,
}

impl FileSpec {
    /// Plik z rozmiarem w MiB.
    pub fn new(name: &str, url: &str, size_mib: u64, sha256: Option<&str>) -> Self {
        Self {
            name: name.into(),
            url: url.into(),
            size: size_mib * MIB,
            sha256: sha256.map(str::to_ascii_lowercase),
        }
    }

    /// Twardy limit pobierania (2× rozmiar z katalogu, co najmniej +64 MiB).
    pub fn limit(&self) -> u64 {
        self.size.saturating_mul(2).max(self.size + 64 * MIB)
    }
}

/// Wpis wybierany z archiwum.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pick {
    /// Ścieżka wpisu w archiwum.
    pub member: String,
    /// Ścieżka docelowa (względna w katalogu pozycji).
    pub dest: String,
    /// Przypięty SHA-256 wpisu.
    pub sha256: Option<String>,
}

/// Sposób instalacji.
#[derive(Debug, Clone, PartialEq)]
pub enum Install {
    /// Każdy plik → `<katalog>/<nazwa>`.
    Files,
    /// Model GGUF `providers-local`: plik + `<plik>.sha256` (rekord rozpoznawany przez dostawcę).
    Gguf,
    /// Embedder `lib-embed`: pliki + `embed.json` z SHA-256 (szablon manifestu bez hashy).
    Embed(Box<EmbedManifest>),
    /// Model mówcy: pliki + manifest `alfa-speaker-v1` (szablon; `sha256` = hash pierwszego pliku).
    Speaker {
        /// Nazwa pliku manifestu.
        manifest: String,
        /// Szablon JSON.
        template: serde_json::Value,
    },
    /// Jedno archiwum ZIP → wybrane wpisy.
    Pick(Vec<Pick>),
    /// Jedno archiwum ZIP → całe drzewo (bez prefiksu `strip`) z wymaganymi plikami.
    Tree {
        /// Usuwany prefiks wpisów (np. `Release/`); wpisy spoza niego są pomijane.
        strip: String,
        /// Pliki wymagane po rozpakowaniu.
        require: Vec<String>,
    },
    /// Instalacja ręczna (opis w `note`); obecność plików = zainstalowana.
    Manual(Vec<String>),
}

/// Pozycja katalogu.
#[derive(Debug, Clone, PartialEq)]
pub struct ItemSpec {
    /// Identyfikator (komendy UI; dla embeddera = wartość `[search.embedder] model`).
    pub id: String,
    /// Rodzaj.
    pub kind: ModelItemKind,
    /// Nazwa wyświetlana.
    pub name: String,
    /// Licencja.
    pub license: String,
    /// Strona źródła.
    pub source: String,
    /// Korzeń.
    pub root: Root,
    /// Podkatalog (pusty — sam korzeń, np. GGUF w `models`).
    pub dir: String,
    /// Pliki do pobrania.
    pub files: Vec<FileSpec>,
    /// Instalacja.
    pub install: Install,
    /// Adresy, rozmiary i licencja potwierdzone przez człowieka.
    pub confirmed: bool,
    /// Opis dla UI (PL/EN).
    pub note: LocalizedText,
    /// Systemy i procesory, pod które zbudowano pliki (pusta lista — każdy: modele i wagi).
    pub targets: Vec<Target>,
}

/// Pliki towarzyszące (tekst licencji wag) — pobierane z pozycją, nie liczą się do jej stanu.
pub const COMPANIONS: &[&str] = &["LICENSE"];

/// Czy plik jest towarzyszący ([`COMPANIONS`]).
pub fn is_companion(name: &str) -> bool {
    COMPANIONS.contains(&name)
}

/// Nazwa pliku wykonywalnego dla systemu.
pub fn exe(name: &str) -> String {
    if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.to_owned()
    }
}

impl ItemSpec {
    /// Czy pozycja działa na komputerze `host` ([`target::runs_on`]).
    pub fn runs_on(&self, host: Option<Target>) -> bool {
        target::runs_on(&self.targets, host)
    }

    /// Katalog docelowy na tej maszynie.
    pub fn target(&self, paths: &AppPaths) -> PathBuf {
        let root = match self.root {
            Root::Models => paths.models(),
            Root::Sidecars => paths.sidecars(),
        };
        if self.dir.is_empty() {
            root
        } else {
            root.join(&self.dir)
        }
    }

    /// Bezpieczne nazwy (identyfikator, podkatalog, pliki, wpisy archiwum) — obrona w głąb:
    /// katalog to dane wbudowane, ale ścieżki trafiają do systemu plików.
    pub fn is_safe(&self) -> bool {
        let path_ok = |p: &str| updater_contract::validate_package_path(p).is_ok();
        let outputs_ok = match &self.install {
            Install::Pick(picks) => picks.iter().all(|p| path_ok(&p.dest)),
            Install::Tree { require, .. } | Install::Manual(require) => {
                require.iter().all(|r| path_ok(r))
            }
            _ => true,
        };
        let ok = crate::store::safe_id(&self.id)
            && (self.dir.is_empty() || path_ok(&self.dir))
            && self.files.iter().all(|f| path_ok(&f.name))
            && outputs_ok;
        if !ok {
            tracing::warn!(item = %self.id, "pozycja katalogu z niebezpieczną ścieżką — pominięta");
        }
        ok
    }

    /// Czy pozycję można pobrać w aplikacji.
    pub fn downloadable(&self) -> bool {
        !self.files.is_empty() && !matches!(self.install, Install::Manual(_))
    }

    /// Czy wszystkie hashe są przypięte (pobrania i wybranych wpisów).
    pub fn pinned(&self) -> bool {
        let picks = match &self.install {
            Install::Pick(p) => p.iter().all(|p| p.sha256.is_some()),
            _ => true,
        };
        picks && self.files.iter().all(|f| f.sha256.is_some())
    }

    /// Łączny przybliżony rozmiar.
    pub fn size(&self) -> u64 {
        self.files.iter().map(|f| f.size).sum()
    }

    /// Pliki wynikowe (względne w katalogu pozycji) — ich obecność = pozycja zainstalowana.
    pub fn outputs(&self) -> Vec<String> {
        // Plik towarzyszący (tekst licencji) pobiera się razem z pozycją, ale nie decyduje o jej
        // stanie: kopie zainstalowane przed jego dodaniem nie wyglądają na brakujące (przegląd 21.10).
        let files = || {
            self.files
                .iter()
                .filter(|f| !is_companion(&f.name))
                .map(|f| f.name.clone())
                .collect::<Vec<_>>()
        };
        match &self.install {
            Install::Files | Install::Gguf => files(),
            Install::Embed(_) => {
                let mut out = files();
                out.push(lib_embed::MANIFEST_FILE.into());
                out
            }
            Install::Speaker { manifest, .. } => {
                let mut out = files();
                out.push(manifest.clone());
                out
            }
            Install::Pick(picks) => picks.iter().map(|p| p.dest.clone()).collect(),
            Install::Tree { require, .. } => require.clone(),
            Install::Manual(files) => files.clone(),
        }
    }

    /// Czy wszystkie pliki wynikowe istnieją.
    pub fn present(&self, target: &Path) -> bool {
        let outputs = self.outputs();
        !outputs.is_empty() && outputs.iter().all(|o| target.join(o).is_file())
    }
}

/// Uwaga dla UI.
pub fn note(pl: &str, en: &str) -> LocalizedText {
    LocalizedText::new(pl, en)
}

/// Katalog produkcyjny: modele GGUF z manifestu `providers-local`, embeddery z `lib_embed::CATALOG`
/// i pozycje głosu/sidecarów z [`crate::data`].
pub fn builtin() -> Vec<ItemSpec> {
    builtin_for(Target::host())
}

/// Katalog dla jawnej platformy (np. fixture'y Windows generowane w Cloud Linux).
/// Nieznana platforma otrzymuje tylko pozycje przenośne, jak nieznany host w `builtin`.
pub fn builtin_for(host: Option<Target>) -> Vec<ItemSpec> {
    let mut out = Vec::new();
    match providers_local_impl::builtin_models() {
        Ok(models) => out.extend(models.iter().map(|m| ItemSpec {
            id: m.id.clone(),
            kind: ModelItemKind::Llm,
            name: m.name.clone(),
            license: m.license.clone(),
            source: m.url.clone(),
            root: Root::Models,
            dir: String::new(),
            files: vec![FileSpec {
                name: m.file.clone(),
                url: m.url.clone(),
                size: u64::from(m.size_mb) * MIB,
                sha256: (!m.sha256.is_empty()).then(|| m.sha256.to_ascii_lowercase()),
            }],
            install: Install::Gguf,
            // Potwierdzenie adresu, rozmiaru i licencji przez człowieka — nie wynika z przypiętego
            // hasha (Bielik: Regulamin bielik.ai i licencja modelu bazowego czekają na właściciela).
            confirmed: false,
            targets: Vec::new(),
            note: note(
                "Lokalny model rozmowy (llama.cpp). Wymaga sidecara llama-server.",
                "Local chat model (llama.cpp). Requires the llama-server sidecar.",
            ),
        })),
        Err(e) => tracing::warn!(error = %e, "manifest modeli lokalnych niepoprawny — pomijam"),
    }
    out.extend(lib_embed::CATALOG.iter().map(embed_item));
    out.extend(crate::data::voice_and_sidecars_for(host));
    // Binarki zbudowane pod inny system albo procesor nie trafiają do katalogu (B.6).
    out.retain(|i| i.runs_on(host));
    out
}

/// Pozycja embeddera z katalogu `lib-embed`.
pub fn embed_item(entry: &lib_embed::CatalogEntry) -> ItemSpec {
    let file = |f: &lib_embed::catalog::CatalogFile| {
        FileSpec::new(f.path, f.url, u64::from(f.size_mb), f.sha256)
    };
    let mut template = entry.manifest("", "");
    template.model.sha256.clear();
    template.tokenizer.sha256.clear();
    ItemSpec {
        id: entry.id.into(),
        kind: ModelItemKind::Embed,
        name: entry.name.into(),
        license: entry.license.into(),
        source: entry.homepage.into(),
        root: Root::Models,
        dir: format!("embed/{}", entry.id),
        files: vec![file(&entry.model), file(&entry.tokenizer)],
        install: Install::Embed(Box::new(template)),
        // Jak wyżej: sumy przypięte (2026-10-21), potwierdzenie przez człowieka osobno.
        confirmed: false,
        targets: Vec::new(),
        note: note(
            &format!(
                "Embedder wyszukiwania semantycznego (pamięć F7): {}",
                entry.note
            ),
            "Semantic search embedder (F7 memory). Runs locally on CPU (tract-onnx).",
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_catalog_is_safe_unique_and_https() {
        let items = builtin();
        assert!(items.len() >= 12, "{}", items.len());
        let mut ids: Vec<&str> = items.iter().map(|i| i.id.as_str()).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), items.len(), "powtórzone identyfikatory");
        for item in &items {
            assert!(item.is_safe(), "{}", item.id);
            assert!(!item.outputs().is_empty(), "{}", item.id);
            for f in &item.files {
                assert!(f.url.starts_with("https://"), "{}", f.url);
                let hex = |h: &str| h.len() == 64 && h.chars().all(|c| c.is_ascii_hexdigit());
                assert!(f.sha256.as_deref().is_none_or(hex), "{}", f.name);
            }
            // Potwierdzone = wszystkie hashe przypięte (bez zgody TOFU).
            assert!(!item.confirmed || item.pinned(), "{}", item.id);
            // Każda pozycja do pobrania w aplikacji ma przypięte sumy (C5.1, 2026-10-21) — bez
            // zaufania przy pierwszym użyciu; potwierdzenie przez człowieka (`confirmed`) to osobna sprawa.
            assert!(
                !item.downloadable() || item.pinned(),
                "bez sumy: {}",
                item.id
            );
            if let Install::Embed(template) = &item.install {
                let mut m = (**template).clone();
                m.model.sha256 = "a".repeat(64);
                m.tokenizer.sha256 = "b".repeat(64);
                m.validate().unwrap();
            }
        }
        let vad = items.iter().find(|i| i.id == "silero-vad").unwrap();
        assert!(vad.confirmed && vad.pinned() && vad.downloadable());
        let pocket = items.iter().find(|i| i.id == "sidecar-pocket-tts").unwrap();
        assert!(!pocket.downloadable());
    }

    /// B.6: każda pobierana binarka ma wpisany system i procesor (nowa bez celu nie przejdzie),
    /// modele są wspólne, a katalog zawiera tylko pozycje działające na tym komputerze.
    #[test]
    fn binaries_are_bound_to_a_target_and_catalog_fits_the_host() {
        let host = Target::host();
        for item in builtin() {
            assert!(item.runs_on(host), "{}", item.id);
            let binary = item.kind == ModelItemKind::Sidecar && item.downloadable();
            assert_eq!(binary, !item.targets.is_empty(), "{}", item.id);
        }
        if host == Some(Target::WINDOWS_X64) {
            let ids: Vec<String> = builtin().into_iter().map(|i| i.id).collect();
            assert!(ids.iter().any(|i| i == "sidecar-whisper-cpu"));
            assert!(ids.iter().any(|i| i == "onnxruntime-cpu"));
        }
    }
}
