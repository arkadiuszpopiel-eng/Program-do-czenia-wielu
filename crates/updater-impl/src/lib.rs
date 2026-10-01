//! Implementacja modułu `updater` (docs/modules/updater/SPEC.md, ADR 0007).
//!
//! [`FsUpdater`] — układ `%LOCALAPPDATA%\Alfa\` na systemie plików: `versions\<ver>\`,
//! `current.json` (zapis atomowy), stały `webview-data\`, wybór wersji, przełączenie, rollback,
//! crash-loop, sprzątanie, weryfikacja minisign. Binarium **`alfa`** (`src/bin/alfa.rs`, w tym
//! samym pakiecie — reguła trójki crate'ów zabrania osobnemu pakietowi zależeć od `-impl`)
//! to stały launcher; jego logika: [`launcher`].

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

mod events;
pub mod launcher;
mod store;
mod verify;

use std::path::{Path, PathBuf};

use chrono::Utc;
use core_registry_contract::{ManifestError, ModuleManifest};
use semver::Version;
use updater_contract::{
    AppExit, CrashPolicy, CurrentState, ExitDecision, LaunchChoice, Layout, Release,
    ReleaseManifest, Updater, UpdaterError, choose_version, decide_exit, events as ev,
    prune_victims, select_update,
};

pub use events::Outbox;
pub use verify::public_key;

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
}

impl UpdaterConfig {
    /// Konfiguracja domyślna dla korzenia instalacji.
    pub fn new(root: &Path) -> Self {
        Self {
            root: root.to_path_buf(),
            public_key: None,
            require_version_tag: true,
            crash: CrashPolicy::default(),
        }
    }
}

/// Moduł `updater` na systemie plików.
pub struct FsUpdater {
    layout: Layout,
    config: UpdaterConfig,
    outbox: Outbox,
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

    fn usable(&self, v: &Version) -> bool {
        store::is_usable(&self.layout, v)
    }

    fn save(
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
