//! Kontrakt modułu `updater` (docs/modules/updater/SPEC.md, ADR 0007, PLAN §1.2 „Instalacja”).
//!
//! F1: launcher i układ katalogów — [`Layout`] (`%LOCALAPPDATA%\Alfa\versions\<ver>\`, stały
//! `alfa.exe`, `current.json`, stały `webview-data\`), wybór wersji (aktywna → poprzednia →
//! błąd), atomowe przełączenie, rollback, polityka crash-loop launchera, sprzątanie starych
//! wersji. F3: manifest wydań ([`Release`], [`ReleaseManifest`]), kanał i tryb ([`Channel`],
//! [`UpdateMode`]), port źródła wydań ([`ReleaseFeed`]: manifest + pobieranie z wznawianiem),
//! ochrona przed cofnięciem wersji ([`check_install_allowed`]), reguły paczki ZIP
//! ([`validate_package_path`], [`PackageLimits`]), stan dla UI ([`UpdateStatus`]) i potwierdzenie
//! zdrowego startu nowej wersji ([`AppExit::Unconfirmed`], [`CrashPolicy::confirm_ms`]).
//!
//! Logika deterministyczna (stan, decyzje) jest tu — wspólna dla `-impl` i `-fake`.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

mod error;
mod feed;
mod layout;
mod package;
mod release;
mod state;
mod status;

#[cfg(feature = "contract-tests")]
pub mod contract_tests;

use std::path::{Path, PathBuf};

use schemars::JsonSchema;
use semver::Version;
use serde::{Deserialize, Serialize};

pub use error::UpdaterError;
pub use feed::{
    Channel, DownloadProgress, InstallIntent, MAX_RELEASES, ReleaseFeed, UpdateMode,
    check_install_allowed, is_allowed_url, is_safe_relative, manifest_url, resolve_url,
    validate_manifest,
};
pub use layout::{
    APP_EXE, CURRENT_FILE, LAUNCHER_EXE, Layout, ROOT_DIR, VERSION_FILE, VERSIONS_DIR, WEBVIEW_DIR,
};
pub use package::{MAX_DEPTH, MAX_PATH_BYTES, NOTES_FILE, PackageLimits, validate_package_path};
pub use release::{
    MAX_NOTES_BYTES, RELEASES_SCHEMA, Release, ReleaseManifest, comment_binds_version,
    select_update, version_tag,
};
pub use state::{
    AppExit, Choice, CrashPolicy, CurrentState, ExitDecision, STATE_SCHEMA, choose_version,
    decide_exit, prune_victims,
};
pub use status::{
    ReleaseSummary, UPDATES_FILE, UPDATES_SCHEMA, UpdatePhase, UpdateStatus, UpdatesFile,
    should_show_whats_new,
};

/// Nazwy zdarzeń modułu (Audyt).
pub mod events {
    /// Dostępna aktualizacja (`{ "version" }`).
    pub const AVAILABLE: &str = "updater.available";
    /// Przygotowano wersję: pobrana, zweryfikowana, rozpakowana, aktywna od restartu (`{ "version" }`).
    pub const STAGED: &str = "updater.staged";
    /// Zamieniono launcher `alfa.exe` na wersję z paczki (`{ "version" }`).
    pub const LAUNCHER_REPLACED: &str = "updater.launcher_replaced";
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
