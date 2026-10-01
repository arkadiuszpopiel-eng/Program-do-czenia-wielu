//! Kontrakt modułu `updater` (docs/modules/updater/SPEC.md, ADR 0007, PLAN §1.2 „Instalacja”).
//!
//! F1: launcher i układ katalogów — [`Layout`] (`%LOCALAPPDATA%\Alfa\versions\<ver>\`, stały
//! `alfa.exe`, `current.json`, stały `webview-data\`), wybór wersji (aktywna → poprzednia →
//! błąd), atomowe przełączenie, rollback, polityka crash-loop launchera, sprzątanie starych
//! wersji. Model danych aktualizacji (F3): [`Release`], [`ReleaseManifest`], weryfikacja
//! podpisu minisign i skrótu paczki.
//!
//! Logika deterministyczna (stan, decyzje) jest tu — wspólna dla `-impl` i `-fake`.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

mod error;
mod layout;
mod release;
mod state;

#[cfg(feature = "contract-tests")]
pub mod contract_tests;

use std::path::{Path, PathBuf};

use schemars::JsonSchema;
use semver::Version;
use serde::{Deserialize, Serialize};

pub use error::UpdaterError;
pub use layout::{
    APP_EXE, CURRENT_FILE, LAUNCHER_EXE, Layout, ROOT_DIR, VERSION_FILE, VERSIONS_DIR, WEBVIEW_DIR,
};
pub use release::{
    RELEASES_SCHEMA, Release, ReleaseManifest, comment_binds_version, select_update, version_tag,
};
pub use state::{
    AppExit, Choice, CrashPolicy, CurrentState, ExitDecision, STATE_SCHEMA, choose_version,
    decide_exit, prune_victims,
};

/// Nazwy zdarzeń modułu (Audyt).
pub mod events {
    /// Dostępna aktualizacja (`{ "version" }`).
    pub const AVAILABLE: &str = "updater.available";
    /// Przygotowano wersję (F3) (`{ "version" }`).
    pub const STAGED: &str = "updater.staged";
    /// Przełączono wersję (`{ "from", "to" }`).
    pub const SWITCHED: &str = "updater.switched";
    /// Rollback (`{ "from", "to", "reason" }`).
    pub const ROLLED_BACK: &str = "updater.rolled_back";
    /// Nieprawidłowy podpis lub skrót paczki (`{ "version", "reason" }`).
    pub const SIGNATURE_INVALID: &str = "updater.signature_invalid";
    /// Usunięto stare wersje (`{ "removed" }`).
    pub const PRUNED: &str = "updater.pruned";
}

/// Wersja do uruchomienia przez launcher.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct LaunchChoice {
    /// Wersja.
    #[schemars(with = "String")]
    pub version: Version,
    /// Plik wykonywalny (`versions\<ver>\alfa-desktop.exe`).
    pub exe: PathBuf,
    /// Czy to wersja zapasowa.
    pub fallback: bool,
    /// Powód użycia wersji zapasowej.
    pub reason: Option<String>,
}

/// Kontrakt modułu `updater`. Operacje synchroniczne (pliki); z kodu async — `spawn_blocking`.
pub trait Updater: Send + Sync {
    /// Układ katalogów.
    fn layout(&self) -> &Layout;
    /// Stan `current.json` (`None` — brak albo uszkodzony plik).
    fn state(&self) -> Result<Option<CurrentState>, UpdaterError>;
    /// Zainstalowane, poprawne wersje (rosnąco).
    fn installed(&self) -> Result<Vec<Version>, UpdaterError>;
    /// Wersja do uruchomienia: aktywna → (brak/uszkodzona) poprzednia → błąd.
    fn select_launch(&self) -> Result<LaunchChoice, UpdaterError>;
    /// Przełącza aktywną wersję **atomowo** (zapis pliku tymczasowego + `rename`).
    fn switch_to(&self, version: &Version) -> Result<CurrentState, UpdaterError>;
    /// Rollback = przełączenie na poprzednią; zwraca nową aktywną.
    fn rollback(&self) -> Result<Version, UpdaterError>;
    /// Oznacza aktywną wersję jako dobrą (po zdrowym starcie).
    fn mark_good(&self, version: &Version) -> Result<(), UpdaterError>;
    /// Zapisuje wynik uruchomienia (polityka crash-loop) i zwraca decyzję launchera.
    fn record_exit(&self, version: &Version, exit: &AppExit) -> Result<ExitDecision, UpdaterError>;
    /// Usuwa stare wersje, zostawiając `keep` (zawsze aktywną i poprzednią); zwraca usunięte.
    fn prune(&self, keep: usize) -> Result<Vec<Version>, UpdaterError>;
    /// Wybiera dostępną aktualizację z manifestu wydań (pobieranie manifestu — F3).
    fn check(&self, manifest: &ReleaseManifest) -> Result<Option<Release>, UpdaterError>;
    /// Weryfikuje paczkę wydania: SHA-256 i podpis minisign kluczem z konfiguracji (z wiązaniem
    /// wersji w komentarzu zaufanym). Paczka bez ważnego podpisu nigdy nie jest instalowana.
    fn verify_release(&self, release: &Release, package: &Path) -> Result<(), UpdaterError>;
}
