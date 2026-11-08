//! Raport dzienny Marszałka (liczniki dnia lokalnego, najczęstsze blokady, tekst po polsku).

use std::collections::BTreeMap;

use chrono::NaiveDate;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::Watch;
use super::text::reason_pl;

/// Raport dzienny.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct DailyReport {
    /// Dzień (czas lokalny).
    pub day: Option<NaiveDate>,
    /// Zgłoszone.
    pub submitted: u32,
    /// Ukończone.
    pub succeeded: u32,
    /// Nieudane.
    pub failed: u32,
    /// Anulowane.
    pub cancelled: u32,
    /// Po terminie.
    pub expired: u32,
    /// Przekroczony budżet (zadania albo tła).
    pub budget: u32,
    /// Pominięte (warunek zależności).
    pub skipped: u32,
    /// Eskalacje.
    pub escalations: u32,
    /// Eskalacje pominięte przez limit.
    pub escalations_suppressed: u32,
    /// Powody blokad (powód → liczba).
    pub blockers: BTreeMap<String, u32>,
    /// Tekst po polsku.
    pub text: String,
}

impl Watch {
    /// Raport dnia (czas lokalny strefy nadzoru).
    pub fn report(&self, day: NaiveDate) -> DailyReport {
        let mut r = self.days.get(&day).cloned().unwrap_or_default();
        r.day = Some(day);
        let mut blockers: Vec<(&String, &u32)> = r.blockers.iter().collect();
        blockers.sort_by(|a, b| b.1.cmp(a.1).then(a.0.cmp(b.0)));
        let top: Vec<String> = blockers
            .iter()
            .take(3)
            .map(|(k, n)| format!("{} ({n})", reason_pl(k)))
            .collect();
        r.text = format!(
            "Raport Marszałka za {day}. Zadania — zgłoszone: {}, ukończone: {}, nieudane: {}, anulowane: {}, po terminie: {}, budżet: {}, pominięte: {}. Eskalacje: {}{}.{}",
            r.submitted,
            r.succeeded,
            r.failed,
            r.cancelled,
            r.expired,
            r.budget,
            r.skipped,
            r.escalations,
            if r.escalations_suppressed > 0 {
                format!(" (+{} tylko w raporcie)", r.escalations_suppressed)
            } else {
                String::new()
            },
            if top.is_empty() {
                String::new()
            } else {
                format!(" Najczęstsze blokady: {}.", top.join(", "))
            }
        );
        r
    }
}
