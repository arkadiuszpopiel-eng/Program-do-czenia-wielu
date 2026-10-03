//! Źródło wydań (F3): kanał (stabilny/testowy), tryb aktualizacji, adresy manifestu i paczek
//! (tylko HTTPS; `http://` wyłącznie na pętli zwrotnej w testach), ochrona przed cofnięciem
//! wersji i port [`ReleaseFeed`] (HTTP w `-impl`, pamięć w `-fake`). Bez telemetrii: zapytanie
//! to zwykłe `GET` manifestu kanału, bez identyfikatorów maszyny ani użytkownika.

use std::path::Path;

use async_trait::async_trait;
use schemars::JsonSchema;
use semver::Version;
use serde::{Deserialize, Serialize};

use crate::error::UpdaterError;
use crate::release::{RELEASES_SCHEMA, Release, ReleaseManifest};

/// Maksymalna liczba wydań w manifeście (większy = podejrzany).
pub const MAX_RELEASES: usize = 64;

/// Kanał aktualizacji.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Channel {
    /// Sprawdzone wersje (bez wersji przedpremierowych `-beta`, `-rc`).
    #[default]
    Stable,
    /// Wersje testowe (także przedpremierowe). W ustawieniach UI: „Testowy” (`preview`).
    #[serde(alias = "preview")]
    Beta,
}

impl Channel {
    /// Nazwa w adresie manifestu (`<feed>/<kanał>.json`).
    pub fn as_str(self) -> &'static str {
        match self {
            Channel::Stable => "stable",
            Channel::Beta => "beta",
        }
    }

    /// Kanał z wartości ustawienia (`stable`, `beta`, `preview`); nieznana → stabilny.
    pub fn from_setting(value: &str) -> Self {
        match value {
            "beta" | "preview" => Channel::Beta,
            _ => Channel::Stable,
        }
    }

    /// Czy wersja należy do kanału (stabilny odrzuca wersje przedpremierowe).
    pub fn accepts(self, version: &Version) -> bool {
        self == Channel::Beta || version.pre.is_empty()
    }
}

/// Tryb aktualizacji (ustawienie `updates.mode`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum UpdateMode {
    /// Sprawdza codziennie, pobiera i przygotowuje sama; nowa wersja od następnego startu.
    Auto,
    /// Sprawdza codziennie, pyta przed pobraniem.
    #[default]
    Ask,
    /// Tylko na żądanie („Sprawdź teraz”).
    Manual,
}

impl UpdateMode {
    /// Tryb z wartości ustawienia; nieznana → „pytaj”.
    pub fn from_setting(value: &str) -> Self {
        match value {
            "auto" => UpdateMode::Auto,
            "manual" => UpdateMode::Manual,
            _ => UpdateMode::Ask,
        }
    }

    /// Czy sprawdzać automatycznie (raz na dobę).
    pub fn checks_automatically(self) -> bool {
        self != UpdateMode::Manual
    }
}

/// Po co instalujemy wydanie.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum InstallIntent {
    /// Zwykła aktualizacja — tylko wersja nowsza od bieżącej.
    Update,
    /// Jawny powrót użytkownika do starszego wydania (jedyny przypadek, gdy starsza wersja jest
    /// dozwolona; podpis nadal musi wiązać tę wersję).
    UserRollback,
}

/// Ochrona przed cofnięciem wersji: wersja ≤ bieżącej tylko przy [`InstallIntent::UserRollback`].
pub fn check_install_allowed(
    version: &Version,
    current: &Version,
    intent: InstallIntent,
) -> Result<(), UpdaterError> {
    if intent == InstallIntent::Update && version <= current {
        return Err(UpdaterError::Downgrade {
            version: version.to_string(),
            current: current.to_string(),
        });
    }
    Ok(())
}

/// Postęp pobierania.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
pub struct DownloadProgress {
    /// Pobrane bajty (z wznowioną częścią).
    pub downloaded: u64,
    /// Rozmiar paczki, jeśli serwer go podał.
    pub total: Option<u64>,
    /// Czy pobieranie wznowiono (HTTP Range).
    pub resumed: bool,
}

/// Źródło wydań. Anulowanie = porzucenie przyszłości (częściowy plik zostaje do wznowienia).
#[async_trait]
pub trait ReleaseFeed: Send + Sync {
    /// Manifest kanału (zweryfikowany: schemat, kanał, limit wydań).
    async fn manifest(&self, channel: Channel) -> Result<ReleaseManifest, UpdaterError>;
    /// Pobiera paczkę wydania do `dest`, **wznawiając** od bieżącego rozmiaru pliku (HTTP Range);
    /// serwer bez obsługi zakresów — od początku. Przerwanie = [`UpdaterError::Network`].
    async fn download(
        &self,
        release: &Release,
        dest: &Path,
        progress: &(dyn Fn(DownloadProgress) + Send + Sync),
    ) -> Result<(), UpdaterError>;
}

/// Sprawdza manifest pobrany dla kanału.
pub fn validate_manifest(manifest: &ReleaseManifest, channel: Channel) -> Result<(), UpdaterError> {
    if manifest.schema != RELEASES_SCHEMA {
        return Err(UpdaterError::invalid(format!(
            "nieznany schemat manifestu {}",
            manifest.schema
        )));
    }
    if manifest.channel != channel.as_str() {
        return Err(UpdaterError::invalid(format!(
            "manifest kanału {} zamiast {}",
            manifest.channel,
            channel.as_str()
        )));
    }
    if manifest.releases.len() > MAX_RELEASES {
        return Err(UpdaterError::invalid("za dużo wydań w manifeście"));
    }
    Ok(())
}

/// Host adresu `http(s)://host[:port]/…` (bez portu, nawiasy IPv6 zachowane).
fn host_of(rest: &str) -> &str {
    let authority = rest.split(['/', '?', '#']).next().unwrap_or_default();
    let authority = authority.rsplit('@').next().unwrap_or(authority);
    if authority.starts_with('[') {
        return authority
            .find(']')
            .map_or(authority, |end| &authority[..=end]);
    }
    authority.split(':').next().unwrap_or(authority)
}

/// Czy adres jest dozwolony: `https://` z hostem; `http://` tylko na pętli zwrotnej i tylko gdy
/// `allow_loopback_http` (testy z lokalnym serwerem — nigdy w konfiguracji produkcyjnej).
pub fn is_allowed_url(url: &str, allow_loopback_http: bool) -> bool {
    if url.chars().any(|c| c.is_control() || c.is_whitespace()) || url.len() > 2048 {
        return false;
    }
    if let Some(rest) = url.strip_prefix("https://") {
        return !host_of(rest).is_empty();
    }
    match url.strip_prefix("http://") {
        Some(rest) if allow_loopback_http => {
            matches!(host_of(rest), "127.0.0.1" | "localhost" | "[::1]")
        }
        _ => false,
    }
}

/// Czy odwołanie względne do paczki jest bezpieczne (`alfa-1.2.0-x64.zip`, `pliki/alfa.zip`).
pub fn is_safe_relative(reference: &str) -> bool {
    !reference.is_empty()
        && reference.len() <= 512
        && !reference.starts_with('/')
        && reference
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-' | b'~' | b'/'))
        && reference
            .split('/')
            .all(|s| !s.is_empty() && s != "." && s != "..")
}

/// Adres manifestu kanału: `<feed>/<kanał>.json`.
pub fn manifest_url(
    feed: &str,
    channel: Channel,
    allow_loopback_http: bool,
) -> Result<String, UpdaterError> {
    let feed = feed.trim().trim_end_matches('/');
    if !is_allowed_url(feed, allow_loopback_http) {
        return Err(UpdaterError::NotConfigured {
            reason: "adres wydań musi być https://".to_owned(),
        });
    }
    Ok(format!("{feed}/{}.json", channel.as_str()))
}

/// Adres paczki: bezwzględny `https://` albo względny wobec adresu manifestu.
pub fn resolve_url(
    manifest_url: &str,
    reference: &str,
    allow_loopback_http: bool,
) -> Result<String, UpdaterError> {
    if reference.contains("://") {
        return if is_allowed_url(reference, allow_loopback_http) {
            Ok(reference.to_owned())
        } else {
            Err(UpdaterError::invalid("adres paczki nie jest https://"))
        };
    }
    if !is_safe_relative(reference) {
        return Err(UpdaterError::invalid("niebezpieczny adres względny paczki"));
    }
    let base = manifest_url
        .rsplit_once('/')
        .map_or(manifest_url, |(dir, _)| dir);
    Ok(format!("{base}/{reference}"))
}

#[cfg(test)]
#[path = "feed_tests.rs"]
mod tests;
