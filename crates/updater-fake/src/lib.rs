//! Atrapa modułu `updater` (docs/modules/updater/SPEC.md, „Fake”): wersje jako wpisy w pamięci,
//! `switch_to` bez restartu, repozytorium wydań z fixture'ami (dobre i złe podpisy), wirtualny
//! zegar. Logika stanu (wybór, rollback, crash-loop, sprzątanie) — ta sama co w `-impl`
//! (`updater-contract`).

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard, PoisonError};

use chrono::{DateTime, Duration, TimeZone, Utc};
use semver::Version;
use sha2::Digest;
use updater_contract::{
    AppExit, CrashPolicy, CurrentState, ExitDecision, LaunchChoice, Layout, Release,
    ReleaseManifest, Updater, UpdaterError, choose_version, comment_binds_version, decide_exit,
    prune_victims, select_update,
};

/// Klucz „publiczny” atrapy (podpisy fixture'ów).
pub const FAKE_KEY: &str = "klucz-atrapy";

#[derive(Debug, Default)]
struct Inner {
    /// Wersja → czy poprawna (`false` = uszkodzona).
    installed: BTreeMap<Version, bool>,
    state: Option<CurrentState>,
    packages: BTreeMap<PathBuf, Vec<u8>>,
    ticks: i64,
    switches: Vec<Version>,
    fail_next: Option<UpdaterError>,
}

/// Atrapa `updater` w pamięci.
pub struct FakeUpdater {
    layout: Layout,
    policy: CrashPolicy,
    inner: Mutex<Inner>,
}

impl Default for FakeUpdater {
    fn default() -> Self {
        Self::new()
    }
}

/// SHA-256 (hex).
pub fn sha256_hex(data: &[u8]) -> String {
    sha2::Sha256::digest(data)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// Podpis atrapy: `atrapa|<klucz>|<komentarz zaufany>|<sha treści>`.
pub fn fake_signature(key: &str, trusted_comment: &str, data: &[u8]) -> String {
    format!("atrapa|{key}|{trusted_comment}|{}", sha256_hex(data))
}

impl FakeUpdater {
    /// Atrapa z korzeniem `C:\Alfa` (tylko ścieżki — nic nie powstaje na dysku).
    pub fn new() -> Self {
        Self {
            layout: Layout::new(PathBuf::from("C:\\Alfa")),
            policy: CrashPolicy::default(),
            inner: Mutex::new(Inner::default()),
        }
    }

    fn lock(&self) -> MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn now(inner: &mut Inner) -> DateTime<Utc> {
        inner.ticks += 1;
        Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0)
            .single()
            .unwrap_or_default()
            + Duration::seconds(inner.ticks)
    }

    fn take_failure(inner: &mut Inner) -> Result<(), UpdaterError> {
        inner.fail_next.take().map_or(Ok(()), Err)
    }

    /// Instaluje poprawną wersję.
    pub fn install(&self, version: &Version) {
        self.lock().installed.insert(version.clone(), true);
    }

    /// Psuje wersję (np. brak pliku wykonywalnego).
    pub fn corrupt(&self, version: &Version) {
        if let Some(ok) = self.lock().installed.get_mut(version) {
            *ok = false;
        }
    }

    /// Rejestruje paczkę wydania pod ścieżką.
    pub fn add_package(&self, path: &Path, bytes: &[u8]) {
        self.lock()
            .packages
            .insert(path.to_path_buf(), bytes.to_vec());
    }

    /// Wydanie z poprawnym podpisem atrapy dla treści `data`.
    pub fn signed_release(version: &Version, data: &[u8]) -> Release {
        Release {
            version: version.clone(),
            url: format!("https://repo.atrapa/alfa-{version}.zip"),
            sha256: sha256_hex(data),
            minisign: fake_signature(FAKE_KEY, &format!("version:{version}"), data),
            notes: format!("Co nowego w {version}"),
            min_previous: None,
        }
    }

    /// Historia przełączeń (`switch_to`, rollback, crash-loop).
    pub fn switches(&self) -> Vec<Version> {
        self.lock().switches.clone()
    }

    /// Następna operacja zwróci ten błąd (jednorazowo).
    pub fn fail_next(&self, error: UpdaterError) {
        self.lock().fail_next = Some(error);
    }

    fn usable(inner: &Inner, v: &Version) -> bool {
        inner.installed.get(v).copied().unwrap_or(false)
    }
}

fn invalid(reason: &str) -> UpdaterError {
    UpdaterError::SignatureInvalid {
        reason: reason.to_owned(),
    }
}

impl Updater for FakeUpdater {
    fn layout(&self) -> &Layout {
        &self.layout
    }

    fn state(&self) -> Result<Option<CurrentState>, UpdaterError> {
        let mut inner = self.lock();
        Self::take_failure(&mut inner)?;
        Ok(inner.state.clone())
    }

    fn installed(&self) -> Result<Vec<Version>, UpdaterError> {
        let mut inner = self.lock();
        Self::take_failure(&mut inner)?;
        Ok(inner
            .installed
            .iter()
            .filter(|(_, ok)| **ok)
            .map(|(v, _)| v.clone())
            .collect())
    }

    fn select_launch(&self) -> Result<LaunchChoice, UpdaterError> {
        let mut inner = self.lock();
        Self::take_failure(&mut inner)?;
        let all: Vec<Version> = inner.installed.keys().cloned().collect();
        let choice = choose_version(inner.state.as_ref(), &all, &|v| Self::usable(&inner, v))?;
        Ok(LaunchChoice {
            exe: self.layout.app_exe(&choice.version),
            version: choice.version,
            fallback: choice.fallback,
            reason: choice.reason,
        })
    }

    fn switch_to(&self, version: &Version) -> Result<CurrentState, UpdaterError> {
        let mut inner = self.lock();
        Self::take_failure(&mut inner)?;
        if !Self::usable(&inner, version) {
            return Err(UpdaterError::NotInstalled {
                version: version.to_string(),
            });
        }
        let now = Self::now(&mut inner);
        let next = match &inner.state {
            Some(s) => s.switched(version, now),
            None => CurrentState::initial(version.clone(), now),
        };
        inner.state = Some(next.clone());
        inner.switches.push(version.clone());
        Ok(next)
    }

    fn rollback(&self) -> Result<Version, UpdaterError> {
        let mut inner = self.lock();
        Self::take_failure(&mut inner)?;
        let now = Self::now(&mut inner);
        let next = inner
            .state
            .as_ref()
            .ok_or(UpdaterError::NoPrevious)?
            .rolled_back(now)?;
        if !Self::usable(&inner, &next.active) {
            return Err(UpdaterError::NotInstalled {
                version: next.active.to_string(),
            });
        }
        inner.switches.push(next.active.clone());
        inner.state = Some(next.clone());
        Ok(next.active)
    }

    fn mark_good(&self, version: &Version) -> Result<(), UpdaterError> {
        let mut inner = self.lock();
        Self::take_failure(&mut inner)?;
        let now = Self::now(&mut inner);
        inner.state = inner.state.as_ref().map(|s| s.marked_good(version, now));
        Ok(())
    }

    fn record_exit(&self, version: &Version, exit: &AppExit) -> Result<ExitDecision, UpdaterError> {
        let mut inner = self.lock();
        Self::take_failure(&mut inner)?;
        let now = Self::now(&mut inner);
        let state = inner
            .state
            .clone()
            .unwrap_or_else(|| CurrentState::initial(version.clone(), now));
        let previous_usable = state
            .previous
            .as_ref()
            .is_some_and(|p| Self::usable(&inner, p));
        let (next, decision) =
            decide_exit(&state, version, exit, &self.policy, previous_usable, now);
        if let ExitDecision::FallBack { to } = &decision {
            inner.switches.push(to.clone());
        }
        inner.state = Some(next);
        Ok(decision)
    }

    fn prune(&self, keep: usize) -> Result<Vec<Version>, UpdaterError> {
        let mut inner = self.lock();
        Self::take_failure(&mut inner)?;
        let all: Vec<Version> = inner.installed.keys().cloned().collect();
        let victims = prune_victims(&all, inner.state.as_ref(), keep);
        for v in &victims {
            inner.installed.remove(v);
        }
        Ok(victims)
    }

    fn check(&self, manifest: &ReleaseManifest) -> Result<Option<Release>, UpdaterError> {
        let mut inner = self.lock();
        Self::take_failure(&mut inner)?;
        let (current, bad) = match &inner.state {
            Some(s) => (s.active.clone(), s.bad.clone()),
            None => (Version::new(0, 0, 0), Vec::new()),
        };
        Ok(select_update(manifest, &current, &bad).cloned())
    }

    fn verify_release(&self, release: &Release, package: &Path) -> Result<(), UpdaterError> {
        let mut inner = self.lock();
        Self::take_failure(&mut inner)?;
        release.validate()?;
        let data = inner
            .packages
            .get(package)
            .cloned()
            .ok_or_else(|| UpdaterError::io(format!("brak paczki {}", package.display())))?;
        let mut parts = release.minisign.splitn(4, '|');
        let (Some("atrapa"), Some(key), Some(comment), Some(signed_sha)) =
            (parts.next(), parts.next(), parts.next(), parts.next())
        else {
            return Err(invalid("zły format podpisu"));
        };
        if !comment_binds_version(comment, &release.version) {
            return Err(invalid("podpis nie wiąże wersji"));
        }
        if key != FAKE_KEY {
            return Err(invalid("podpis innym kluczem"));
        }
        let actual = sha256_hex(&data);
        if !actual.eq_ignore_ascii_case(&release.sha256) {
            return Err(UpdaterError::HashMismatch);
        }
        if signed_sha != actual {
            return Err(invalid("podpis innej treści"));
        }
        Ok(())
    }
}
