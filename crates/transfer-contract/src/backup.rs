//! Kopie zapasowe = zaplanowany eksport (ten sam kod) z rotacją N ostatnich; snapshoty przed
//! importem rotowane tak samo. Czyste funkcje: nazwy plików, wybór do usunięcia, termin kopii.

use std::path::PathBuf;

use chrono::{DateTime, Duration, NaiveDateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::scope::{CancelToken, ExportScope};
use accounts_hub_contract::SecretString;

/// Prefiks nazw kopii zapasowych.
pub const BACKUP_PREFIX: &str = "alfa-backup-";
/// Prefiks nazw snapshotów.
pub const SNAPSHOT_PREFIX: &str = "snap-";
/// Rozszerzenie paczek.
pub const EXTENSION: &str = "alfa";
/// Domyślna liczba snapshotów (docs/modules/transfer/SPEC.md: `snapshots_keep = 5`).
pub const DEFAULT_SNAPSHOTS_KEEP: usize = 5;

const STAMP: &str = "%Y%m%d-%H%M%S-%3f";

/// Nazwa pliku (bez rozszerzenia) z prefiksem i znacznikiem czasu (sortowalna leksykograficznie).
pub fn stamped_name(prefix: &str, at: DateTime<Utc>) -> String {
    format!("{prefix}{}", at.format(STAMP))
}

/// Znacznik czasu z nazwy (`None`, gdy nazwa nie pasuje do wzorca prefiksu).
pub fn parse_stamp(prefix: &str, stem: &str) -> Option<DateTime<Utc>> {
    let rest = stem.strip_prefix(prefix)?;
    let stamp = rest.get(..19)?;
    NaiveDateTime::parse_from_str(stamp, STAMP)
        .ok()
        .map(|n| n.and_utc())
}

/// Wybór plików do usunięcia przy rotacji: zostaje `keep` najnowszych (po znaczniku w nazwie);
/// nazwy spoza wzorca nie są nigdy dotykane.
pub fn rotation_victims(prefix: &str, stems: &[String], keep: usize) -> Vec<String> {
    let mut dated: Vec<(DateTime<Utc>, &String)> = stems
        .iter()
        .filter_map(|s| parse_stamp(prefix, s).map(|t| (t, s)))
        .collect();
    dated.sort();
    let excess = dated.len().saturating_sub(keep);
    dated
        .into_iter()
        .take(excess)
        .map(|(_, s)| s.clone())
        .collect()
}

/// Harmonogram kopii (F7: cron; w F1 — interwał i reguły tła).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct BackupSchedule {
    /// Odstęp między kopiami (sekundy).
    pub interval_secs: u64,
    /// Nie uruchamiaj na baterii.
    pub skip_on_battery: bool,
    /// Nie uruchamiaj przy pełnym ekranie / trybie gry.
    pub skip_fullscreen: bool,
}

impl Default for BackupSchedule {
    fn default() -> Self {
        Self {
            interval_secs: 24 * 3600,
            skip_on_battery: true,
            skip_fullscreen: true,
        }
    }
}

/// Stan maszyny istotny dla zadań tła.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct BackgroundState {
    /// Zasilanie z baterii.
    pub on_battery: bool,
    /// Aplikacja pełnoekranowa / tryb gry.
    pub fullscreen: bool,
}

impl BackupSchedule {
    /// Czy kopia jest należna i dozwolona teraz.
    pub fn is_due(
        &self,
        last: Option<DateTime<Utc>>,
        now: DateTime<Utc>,
        state: BackgroundState,
    ) -> bool {
        if (self.skip_on_battery && state.on_battery) || (self.skip_fullscreen && state.fullscreen)
        {
            return false;
        }
        let interval = Duration::seconds(i64::try_from(self.interval_secs).unwrap_or(i64::MAX));
        last.is_none_or(|l| now - l >= interval)
    }
}

/// Żądanie kopii zapasowej (eksport `kind = backup` + rotacja).
#[derive(Debug, Clone)]
pub struct BackupRequest {
    /// Katalog kopii (lokalny, sieciowy, OneDrive).
    pub dir: PathBuf,
    /// Zakres (jak eksport; zwykle wszystkie sesje).
    pub scope: ExportScope,
    /// Liczba zachowanych kopii (rotacja; minimum 1).
    pub keep: usize,
    /// Hasło (opcjonalnie — może pochodzić z Credential Managera).
    pub password: Option<SecretString>,
    /// Anulowanie.
    pub cancel: Option<CancelToken>,
}

#[cfg(test)]
mod tests {
    use chrono::TimeZone;

    use super::*;

    fn at(s: i64) -> DateTime<Utc> {
        Utc.timestamp_opt(1_790_000_000 + s, 0)
            .single()
            .unwrap_or_default()
    }

    #[test]
    fn names_round_trip_and_sort() {
        let a = stamped_name(BACKUP_PREFIX, at(0));
        let b = stamped_name(BACKUP_PREFIX, at(3600));
        assert!(a < b);
        assert_eq!(parse_stamp(BACKUP_PREFIX, &a), Some(at(0)));
        assert_eq!(parse_stamp(SNAPSHOT_PREFIX, &a), None);
        assert_eq!(parse_stamp(BACKUP_PREFIX, "alfa-backup-zly"), None);
    }

    #[test]
    fn rotation_keeps_newest_and_ignores_foreign_files() {
        let mut stems: Vec<String> = (0..6)
            .map(|i| stamped_name(BACKUP_PREFIX, at(i * 60)))
            .collect();
        stems.push("moja-kopia".into());
        let victims = rotation_victims(BACKUP_PREFIX, &stems, 4);
        assert_eq!(victims, vec![stems[0].clone(), stems[1].clone()]);
        assert!(rotation_victims(BACKUP_PREFIX, &stems, 10).is_empty());
    }

    #[test]
    fn schedule_respects_background_rules() {
        let s = BackupSchedule::default();
        let idle = BackgroundState::default();
        assert!(s.is_due(None, at(0), idle));
        assert!(!s.is_due(Some(at(0)), at(3600), idle));
        assert!(s.is_due(Some(at(0)), at(24 * 3600), idle));
        let battery = BackgroundState {
            on_battery: true,
            fullscreen: false,
        };
        assert!(!s.is_due(None, at(0), battery));
        let game = BackgroundState {
            on_battery: false,
            fullscreen: true,
        };
        assert!(!s.is_due(None, at(0), game));
    }
}
