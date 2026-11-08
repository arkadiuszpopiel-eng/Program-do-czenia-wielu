//! Stan aktualizacji dla UI („Ustawienia → Aktualizacje”, baner „Uruchom ponownie, aby
//! zaktualizować”, „Co nowego”) i zapis `updates.json` (ostatnie sprawdzenie, pokazane
//! „Co nowego”) — wspólne dla `-impl` i `-fake`.

use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use semver::Version;
use serde::{Deserialize, Serialize};

use crate::feed::{Channel, DownloadProgress, UpdateMode};

/// Plik stanu usługi aktualizacji w katalogu instalacji.
pub const UPDATES_FILE: &str = "updates.json";
/// Wersja schematu `updates.json`.
pub const UPDATES_SCHEMA: u32 = 1;

/// Etap aktualizacji.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum UpdatePhase {
    /// Brak adresu wydań albo klucza publicznego (build deweloperski).
    Disabled,
    /// Jeszcze nie sprawdzano.
    #[default]
    Idle,
    /// Pobieranie manifestu kanału.
    Checking,
    /// Brak nowszej wersji.
    UpToDate,
    /// Jest nowsza wersja (tryb „pytaj” — czeka na zgodę).
    Available,
    /// Pobieranie paczki (z wznawianiem).
    Downloading,
    /// SHA-256 i podpis minisign.
    Verifying,
    /// Rozpakowanie do `versions\<ver>\` i przełączenie `current.json`.
    Installing,
    /// Gotowa — wymaga ponownego uruchomienia.
    Ready,
    /// Błąd (szczegóły w `error`); przerwane pobieranie można wznowić.
    Failed,
}

/// Wydanie widoczne w UI.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ReleaseSummary {
    /// Wersja.
    #[schemars(with = "String")]
    pub version: Version,
    /// „Co nowego” z manifestu (zwykły tekst/Markdown — UI pokazuje jako tekst).
    pub notes: String,
}

/// Stan usługi aktualizacji.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct UpdateStatus {
    /// Etap.
    pub phase: UpdatePhase,
    /// Uruchomiona wersja.
    #[schemars(with = "String")]
    pub current: Version,
    /// Kanał.
    pub channel: Channel,
    /// Tryb.
    pub mode: UpdateMode,
    /// Znalezione wydanie (od `Available` do `Ready`).
    pub available: Option<ReleaseSummary>,
    /// Postęp pobierania (także częściowy plik czekający na wznowienie).
    pub progress: Option<DownloadProgress>,
    /// Wersja przygotowana do uruchomienia po restarcie.
    #[schemars(with = "Option<String>")]
    pub ready: Option<Version>,
    /// Poprzednia wersja (cel „Przywróć poprzednią wersję”).
    #[schemars(with = "Option<String>")]
    pub previous: Option<Version>,
    /// Ostatnie sprawdzenie.
    pub last_check: Option<DateTime<Utc>>,
    /// Opis błędu (`Failed`) albo powód wyłączenia (`Disabled`).
    pub error: Option<String>,
}

impl UpdateStatus {
    /// Stan początkowy.
    pub fn new(current: Version, channel: Channel, mode: UpdateMode) -> Self {
        Self {
            phase: UpdatePhase::Idle,
            current,
            channel,
            mode,
            available: None,
            progress: None,
            ready: None,
            previous: None,
            last_check: None,
            error: None,
        }
    }

    /// Czy trwa operacja (sprawdzanie, pobieranie, weryfikacja, instalacja).
    pub fn busy(&self) -> bool {
        matches!(
            self.phase,
            UpdatePhase::Checking
                | UpdatePhase::Downloading
                | UpdatePhase::Verifying
                | UpdatePhase::Installing
        )
    }
}

/// Zawartość `updates.json` (odporna na brak/uszkodzenie — wtedy wartości domyślne).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
pub struct UpdatesFile {
    /// Wersja schematu.
    #[serde(default)]
    pub schema: u32,
    /// Ostatnie udane sprawdzenie.
    #[serde(default)]
    pub last_check: Option<DateTime<Utc>>,
    /// Wersja, dla której pokazano (albo pominięto przy pierwszej instalacji) „Co nowego”.
    #[serde(default)]
    #[schemars(with = "Option<String>")]
    pub whats_new_seen: Option<Version>,
}

/// Czy pokazać „Co nowego” dla uruchomionej wersji: tylko po aktualizacji (była wcześniej
/// widziana starsza wersja), raz. Pierwsza instalacja → nie (zapisujemy bieżącą jako widzianą).
pub fn should_show_whats_new(seen: Option<&Version>, running: &Version) -> bool {
    seen.is_some_and(|s| running > s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn whats_new_once_after_update_only() {
        let v = |s: &str| Version::parse(s).unwrap();
        assert!(
            !should_show_whats_new(None, &v("1.0.0")),
            "pierwsza instalacja"
        );
        assert!(should_show_whats_new(Some(&v("1.0.0")), &v("1.1.0")));
        assert!(
            !should_show_whats_new(Some(&v("1.1.0")), &v("1.1.0")),
            "już pokazane"
        );
        assert!(
            !should_show_whats_new(Some(&v("1.2.0")), &v("1.1.0")),
            "po rollbacku nie"
        );
        let mut s = UpdateStatus::new(v("1.0.0"), Channel::Stable, UpdateMode::Ask);
        assert!(!s.busy());
        s.phase = UpdatePhase::Downloading;
        assert!(s.busy());
        let file: UpdatesFile = serde_json::from_str("{}").unwrap();
        assert_eq!(file, UpdatesFile::default());
    }
}
