//! Implementacja modułu `updater` (docs/modules/updater/SPEC.md, ADR 0007).
//!
//! [`FsUpdater`] — układ `%LOCALAPPDATA%\Alfa\` na systemie plików: `versions\<ver>\`,
//! `current.json` (zapis atomowy), stały `webview-data\`, wybór wersji, przełączenie, rollback,
//! crash-loop, sprzątanie, weryfikacja minisign. F3: [`HttpFeed`] (manifest kanału i pobieranie
//! z wznawianiem przez HTTPS), [`install`] (rozpakowanie paczki z ochroną przed path traversal
//! i zip-bomb), [`UpdateService`] (pełny cykl: sprawdź → pobierz → zweryfikuj → rozpakuj →
//! przełącz → posprzątaj), [`selfupdate`] (bezpieczna zamiana `alfa.exe`), [`WatchdogSignal`]
//! (rollback wersji zlecony przez watchdoga). Binarium **`alfa`** (`src/bin/alfa.rs`, w tym
//! samym pakiecie — reguła trójki crate'ów zabrania osobnemu pakietowi zależeć od `-impl`)
//! to stały launcher; jego logika: [`launcher`] i [`entry`].

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

mod download;
pub mod entry;
mod events;
mod http;
pub mod install;
pub mod instance;
pub mod launcher;
mod ops;
pub mod selfupdate;
mod service;
mod store;
mod verify;
mod watchdog;

use std::path::{Path, PathBuf};

use chrono::Utc;
use core_registry_contract::{ManifestError, ModuleManifest};
use semver::Version;
use updater_contract::{
    AppExit, CrashPolicy, CurrentState, ExitDecision, LaunchChoice, Layout, PackageLimits, Release,
    ReleaseManifest, Updater, UpdaterError, choose_version, decide_exit, events as ev,
    prune_victims, select_update,
};

pub use download::STAGING_DIR;
pub use events::Outbox;
pub use http::HttpFeed;
pub use service::{ServiceOptions, StatusListener, UpdateService};
pub use store::{read_updates_file, write_updates_file};
pub use verify::public_key;
pub use watchdog::WatchdogSignal;

/// Klucz publiczny minisign wbudowany przy kompilacji wydania (`ALFA_UPDATE_PUBKEY`, base64
/// `RW…`); w buildzie deweloperskim brak — aktualizacje wyłączone.
pub const BUILTIN_PUBLIC_KEY: Option<&str> = option_env!("ALFA_UPDATE_PUBKEY");
/// Adres wydań wbudowany przy kompilacji wydania (`ALFA_UPDATE_FEED`, `https://…`).
pub const BUILTIN_FEED_URL: Option<&str> = option_env!("ALFA_UPDATE_FEED");
/// Ile wersji zostaje po sprzątaniu (aktywna + poprzednia).
pub const KEEP_VERSIONS: usize = 2;

/// Treść `module.toml` tego modułu.
pub const MODULE_TOML: &str = include_str!("../module.toml");

/// Konfiguracja (`[updates]` w TOML).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpdaterConfig {
    /// Korzeń instalacji (`%LOCALAPPDATA%\Alfa`).
    pub root: PathBuf,
    /// Klucz publiczny minisign (`RW…` albo pełny plik `.pub`); bez klucza aktualizacje wyłączone.
    pub public_key: Option<String>,
    /// Wymagaj `version:<wersja>` w komentarzu zaufanym podpisu (ochrona przed cofnięciem wersji).
    pub require_version_tag: bool,
    /// Polityka crash-loop launchera.
    pub crash: CrashPolicy,
    /// Adres wydań (`<feed>/<kanał>.json`); bez adresu aktualizacje wyłączone.
    pub feed_url: Option<String>,
    /// Ile wersji zostaje po sprzątaniu.
    pub keep_versions: usize,
    /// Limity rozpakowania paczki.
    pub limits: PackageLimits,
    /// `http://` na pętli zwrotnej (wyłącznie testy z lokalnym serwerem).
    pub allow_loopback_http: bool,
}

impl UpdaterConfig {
    /// Konfiguracja domyślna dla korzenia instalacji (klucz i adres wydań wbudowane w wydanie).
    pub fn new(root: &Path) -> Self {
        Self {
            root: root.to_path_buf(),
            public_key: BUILTIN_PUBLIC_KEY.map(str::to_owned),
            require_version_tag: true,
            crash: CrashPolicy::default(),
            feed_url: BUILTIN_FEED_URL.map(str::to_owned),
            keep_versions: KEEP_VERSIONS,
            limits: PackageLimits::default(),
            allow_loopback_http: false,
        }
    }

    /// Powód wyłączenia aktualizacji (brak adresu wydań albo klucza), `None` — skonfigurowane.
    pub fn disabled_reason(&self) -> Option<String> {
        if self.feed_url.as_deref().is_none_or(|u| u.trim().is_empty()) {
            return Some("brak adresu wydań (build deweloperski)".to_owned());
        }
        if self
            .public_key
            .as_deref()
            .is_none_or(|k| k.trim().is_empty())
        {
            return Some("brak klucza publicznego minisign (build deweloperski)".to_owned());
        }
        None
    }
}

/// Moduł `updater` na systemie plików.
pub struct FsUpdater {
    layout: Layout,
    config: UpdaterConfig,
    pub(crate) outbox: Outbox,
    manifest: ModuleManifest,
}

fn manifest_err(e: ManifestError) -> UpdaterError {
    UpdaterError::invalid(format!("module.toml: {e}"))
}

impl FsUpdater {
    /// Moduł dla konfiguracji (katalogi tworzone przy pierwszym zapisie).
    pub fn new(config: UpdaterConfig) -> Result<Self, UpdaterError> {
        Ok(Self {
            layout: Layout::new(&config.root),
            manifest: ModuleManifest::parse_toml(MODULE_TOML).map_err(manifest_err)?,
            outbox: Outbox::default(),
            config,
        })
    }

    /// Konfiguracja.
    pub fn config(&self) -> &UpdaterConfig {
        &self.config
    }

    /// Tworzy stałe katalogi (`versions\`, `webview-data\`).
    pub fn ensure_layout(&self) -> Result<(), UpdaterError> {
        std::fs::create_dir_all(&self.layout.versions)?;
        std::fs::create_dir_all(&self.layout.webview_data)?;
        Ok(())
    }

    pub(crate) fn usable(&self, v: &Version) -> bool {
        store::is_usable(&self.layout, v)
    }

    pub(crate) fn save(
        &self,
        before: Option<&CurrentState>,
        after: &CurrentState,
    ) -> Result<(), UpdaterError> {
        if before != Some(after) {
            store::write_state(&self.layout, after)?;
        }
        Ok(())
    }
}

impl Updater for FsUpdater {
    fn layout(&self) -> &Layout {
        &self.layout
    }

    fn state(&self) -> Result<Option<CurrentState>, UpdaterError> {
        Ok(store::read_state(&self.layout))
    }

    fn installed(&self) -> Result<Vec<Version>, UpdaterError> {
        Ok(store::version_dirs(&self.layout)
            .into_iter()
            .filter(|v| self.usable(v))
            .collect())
    }

    fn select_launch(&self) -> Result<LaunchChoice, UpdaterError> {
        let state = store::read_state(&self.layout);
        let installed = store::version_dirs(&self.layout);
        let choice = choose_version(state.as_ref(), &installed, &|v| self.usable(v))?;
        Ok(LaunchChoice {
            exe: self.layout.app_exe(&choice.version),
            version: choice.version,
            fallback: choice.fallback,
            reason: choice.reason,
        })
    }

    fn switch_to(&self, version: &Version) -> Result<CurrentState, UpdaterError> {
        if !self.usable(version) {
            return Err(UpdaterError::NotInstalled {
                version: version.to_string(),
            });
        }
        let before = store::read_state(&self.layout);
        let after = match &before {
            Some(s) => s.switched(version, Utc::now()),
            None => CurrentState::initial(version.clone(), Utc::now()),
        };
        self.save(before.as_ref(), &after)?;
        let from = before.map(|s| s.active.to_string());
        self.outbox.emit(
            ev::SWITCHED,
            serde_json::json!({ "from": from, "to": version.to_string() }),
        );
        Ok(after)
    }

    fn rollback(&self) -> Result<Version, UpdaterError> {
        let before = store::read_state(&self.layout).ok_or(UpdaterError::NoPrevious)?;
        let after = before.rolled_back(Utc::now())?;
        if !self.usable(&after.active) {
            return Err(UpdaterError::NotInstalled {
                version: after.active.to_string(),
            });
        }
        self.save(Some(&before), &after)?;
        let payload = serde_json::json!({ "from": before.active.to_string(), "to": after.active.to_string(), "reason": "ręcznie" });
        self.outbox.emit(ev::ROLLED_BACK, payload);
        Ok(after.active)
    }

    fn mark_good(&self, version: &Version) -> Result<(), UpdaterError> {
        if let Some(before) = store::read_state(&self.layout) {
            self.save(Some(&before), &before.marked_good(version, Utc::now()))?;
            if before.active == *version && before.pending {
                // Nowy launcher z paczki — dopiero po zdrowym starcie wersji (zamiana przy
                // następnym starcie, po samoteście). Błąd nie cofa `mark_good`.
                if let Err(e) = selfupdate::stage_launcher(&self.layout, version) {
                    tracing::warn!(error = %e, "nie przygotowano nowego launchera");
                }
            }
        }
        Ok(())
    }

    fn record_exit(&self, version: &Version, exit: &AppExit) -> Result<ExitDecision, UpdaterError> {
        let before = store::read_state(&self.layout);
        let state = before
            .clone()
            .unwrap_or_else(|| CurrentState::initial(version.clone(), Utc::now()));
        let previous_usable = state.previous.as_ref().is_some_and(|p| self.usable(p));
        let (after, decision) = decide_exit(
            &state,
            version,
            exit,
            &self.config.crash,
            previous_usable,
            Utc::now(),
        );
        self.save(before.as_ref(), &after)?;
        if let ExitDecision::FallBack { to } = &decision {
            let payload = serde_json::json!({ "from": version.to_string(), "to": to.to_string(), "reason": "crash-loop" });
            self.outbox.emit(ev::ROLLED_BACK, payload);
        }
        Ok(decision)
    }

    fn prune(&self, keep: usize) -> Result<Vec<Version>, UpdaterError> {
        let state = store::read_state(&self.layout);
        let victims = prune_victims(&store::version_dirs(&self.layout), state.as_ref(), keep);
        for v in &victims {
            store::remove_version(&self.layout, v)?;
        }
        if !victims.is_empty() {
            let removed: Vec<String> = victims.iter().map(ToString::to_string).collect();
            self.outbox
                .emit(ev::PRUNED, serde_json::json!({ "removed": removed }));
        }
        Ok(victims)
    }

    fn check(&self, manifest: &ReleaseManifest) -> Result<Option<Release>, UpdaterError> {
        let state = store::read_state(&self.layout);
        let current = match &state {
            Some(s) => s.active.clone(),
            None => self
                .installed()?
                .into_iter()
                .max()
                .unwrap_or(Version::new(0, 0, 0)),
        };
        let bad = state.map(|s| s.bad).unwrap_or_default();
        let found = select_update(manifest, &current, &bad).cloned();
        if let Some(r) = &found {
            self.outbox.emit(
                ev::AVAILABLE,
                serde_json::json!({ "version": r.version.to_string() }),
            );
        }
        Ok(found)
    }

    fn verify_release(&self, release: &Release, package: &Path) -> Result<(), UpdaterError> {
        let result = verify::verify(
            release,
            package,
            self.config.public_key.as_deref(),
            self.config.require_version_tag,
        );
        if let Err(e) = &result {
            let payload = serde_json::json!({ "version": release.version.to_string(), "reason": e.to_string() });
            self.outbox.emit(ev::SIGNATURE_INVALID, payload);
        }
        result
    }
}
