//! Testy podzespołów: rodzaje, wyniki z miarami i ocena. Testy nie zmieniają systemu — dysk jest
//! sprawdzany na pliku tymczasowym Alfy (usuwanym zawsze, także po błędzie), pamięć na buforze
//! w procesie, sieć połączeniami TCP bez wysyłania danych.
//!
//! Pola nazwane wprost (jednostka w nazwie: `_mb`, `_mhz`, `_pct`, `_c`) — opis wariantów i jednostek
//! jest w `docs/modules/pc-diag/SPEC.md`, nie przy każdym polu.

#![allow(missing_docs)]

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Rodzaj testu.
#[derive(
    Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(tag = "test", content = "target", rename_all = "snake_case")]
pub enum CheckKind {
    /// Procesor: jeden wątek.
    CpuSingle,
    /// Procesor: wszystkie wątki (skalowanie).
    CpuMulti,
    /// Pamięć: zapis wzorca z weryfikacją i przepustowość kopiowania.
    Memory,
    /// Dysk (litera woluminu, np. `C:`): zapis i odczyt sekwencyjny, odczyt losowy 4 KB.
    Disk(String),
    /// Sieć: DNS i czas połączenia z kilkoma serwerami.
    Network,
    /// Spójność: kondycja dysków, wolne miejsce, zdarzenia awarii, bateria, temperatury
    /// (ocena ostatniego zapisu parametrów, bez obciążania sprzętu).
    Integrity,
}

impl CheckKind {
    /// Klucz stały (historia, porównania): `cpu_single`, `disk:C:`…
    pub fn key(&self) -> String {
        match self {
            Self::CpuSingle => "cpu_single".into(),
            Self::CpuMulti => "cpu_multi".into(),
            Self::Memory => "memory".into(),
            Self::Disk(letter) => format!("disk:{letter}"),
            Self::Network => "network".into(),
            Self::Integrity => "integrity".into(),
        }
    }

    /// Z klucza.
    pub fn from_key(key: &str) -> Option<Self> {
        Some(match key {
            "cpu_single" => Self::CpuSingle,
            "cpu_multi" => Self::CpuMulti,
            "memory" => Self::Memory,
            "network" => Self::Network,
            "integrity" => Self::Integrity,
            other => {
                let letter = other.strip_prefix("disk:")?;
                let ok = letter.len() == 2
                    && letter.ends_with(':')
                    && letter.starts_with(|c: char| c.is_ascii_alphabetic());
                if !ok {
                    return None;
                }
                Self::Disk(letter.to_ascii_uppercase())
            }
        })
    }
}

/// Wynik testu.
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum Verdict {
    Pass,
    Warn,
    Fail,
    /// Nie wykonano (np. za mało miejsca, brak sieci) — z powodem w `notes`.
    #[default]
    Skipped,
}

/// Zmierzona wartość.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Metric {
    /// Klucz (`seq_write_mbps`, `latency_ms`…).
    pub key: String,
    /// Etykieta po polsku.
    pub label: String,
    pub value: f64,
    pub unit: String,
    /// Większa wartość jest lepsza.
    pub higher_is_better: bool,
}

impl Metric {
    /// Miara.
    pub fn new(key: &str, label: &str, value: f64, unit: &str, higher_is_better: bool) -> Self {
        Self {
            key: key.into(),
            label: label.into(),
            value,
            unit: unit.into(),
            higher_is_better,
        }
    }
}

/// Wynik jednego testu.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct CheckResult {
    pub kind: CheckKind,
    pub verdict: Verdict,
    pub metrics: Vec<Metric>,
    /// Uwagi i powody oceny (po polsku).
    pub notes: Vec<String>,
    pub duration_ms: u64,
}

impl CheckResult {
    /// Pominięty z powodem.
    pub fn skipped(kind: CheckKind, reason: impl Into<String>) -> Self {
        Self {
            kind,
            verdict: Verdict::Skipped,
            metrics: Vec::new(),
            notes: vec![reason.into()],
            duration_ms: 0,
        }
    }

    /// Wartość miary.
    pub fn metric(&self, key: &str) -> Option<f64> {
        self.metrics.iter().find(|m| m.key == key).map(|m| m.value)
    }
}

/// Przebieg testów (zapisywany w historii).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct CheckRun {
    /// RFC 3339.
    pub at: String,
    pub results: Vec<CheckResult>,
    /// Najgorszy wynik (bez pominiętych; same pominięte — `Skipped`).
    pub overall: Verdict,
}

impl CheckRun {
    /// Przebieg z wynikami i oceną zbiorczą.
    pub fn new(at: String, results: Vec<CheckResult>) -> Self {
        let overall = results
            .iter()
            .map(|r| r.verdict)
            .filter(|v| *v != Verdict::Skipped)
            .max()
            .unwrap_or(Verdict::Skipped);
        Self {
            at,
            results,
            overall,
        }
    }
}
