//! Metryki F2 (docs/ACCEPTANCE.md §5): WER PL, recall „stop/anuluj” z reakcją, precision
//! backchannelu, fałszywe przerwania na godzinę, dokładność usłyszanego prefiksu (±1 słowo),
//! trafność intencji przerwań per klasa.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use voice_cmd_contract::CommandKind;

use crate::eval::manifest::{ItemKind, ManifestEntry};
use crate::eval::results::ItemResult;

/// Licznik trafień.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Ratio {
    /// Trafienia.
    pub hits: usize,
    /// Wszystkie.
    pub total: usize,
}

impl Ratio {
    /// Wartość 0–1 (`None` bez prób).
    pub fn value(&self) -> Option<f64> {
        (self.total > 0).then(|| self.hits as f64 / self.total as f64)
    }

    fn add(&mut self, hit: bool) {
        self.total += 1;
        self.hits += usize::from(hit);
    }
}

/// Normalizacja do WER PL: małe litery, bez interpunkcji, polskie znaki zachowane.
pub fn normalize_words(text: &str) -> Vec<String> {
    text.to_lowercase()
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '\'' {
                c
            } else {
                ' '
            }
        })
        .collect::<String>()
        .split_whitespace()
        .map(str::to_owned)
        .collect()
}

/// Odległość edycyjna na słowach (S + D + I) i długość referencji.
pub fn word_errors(reference: &str, hypothesis: &str) -> (usize, usize) {
    let r = normalize_words(reference);
    let h = normalize_words(hypothesis);
    let mut prev: Vec<usize> = (0..=h.len()).collect();
    for (i, rw) in r.iter().enumerate() {
        let mut cur = vec![i + 1; h.len() + 1];
        for (j, hw) in h.iter().enumerate() {
            let sub = prev[j] + usize::from(rw != hw);
            cur[j + 1] = sub.min(prev[j + 1] + 1).min(cur[j] + 1);
        }
        prev = cur;
    }
    (prev[h.len()], r.len())
}

/// Percentyl (najbliższa ranga) z kopii.
pub fn percentile(values: &[u64], p: f64) -> Option<u64> {
    let mut v = values.to_vec();
    v.sort_unstable();
    let n = v.len();
    if n == 0 {
        return None;
    }
    let rank = ((p / 100.0) * n as f64).ceil() as usize;
    v.get(rank.clamp(1, n) - 1).copied()
}

/// Wynik oceny F2.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct F2Report {
    /// Błędy słów / słowa referencji (WER).
    pub wer_errors: usize,
    /// Słowa referencji.
    pub wer_words: usize,
    /// Recall „stop/anuluj”.
    pub stop_recall: Ratio,
    /// Reakcje na „stop/anuluj” (ms od początku słowa).
    pub stop_reactions_ms: Vec<u64>,
    /// Precision backchannelu: nieprzerwane / (nieprzerwane backchannele + nieprzerwane przerwania).
    pub backchannel_kept: usize,
    /// Przerwania prawdziwe uznane za backchannel (fałszywie nieprzerwane).
    pub interruptions_missed: usize,
    /// Backchannele, które przerwały (do diagnostyki).
    pub backchannels_interrupting: usize,
    /// Prefiks ±1 słowo.
    pub prefix: Ratio,
    /// Intencje per klasa.
    pub intents: BTreeMap<String, Ratio>,
    /// Fałszywe przerwania na godzinę (z dziennika sesji).
    pub false_per_hour: Option<f64>,
    /// Pozycje bez wyniku.
    pub missing: Vec<String>,
}

impl F2Report {
    /// WER (0–1).
    pub fn wer(&self) -> Option<f64> {
        (self.wer_words > 0).then(|| self.wer_errors as f64 / self.wer_words as f64)
    }

    /// Precision backchannelu (0–1).
    pub fn backchannel_precision(&self) -> Option<f64> {
        let predicted = self.backchannel_kept + self.interruptions_missed;
        (predicted > 0).then(|| self.backchannel_kept as f64 / predicted as f64)
    }
}

fn intent_key(intent: voice_dialog_contract::InterruptIntent) -> String {
    serde_json::to_value(intent)
        .ok()
        .and_then(|v| v.as_str().map(str::to_owned))
        .unwrap_or_default()
}

/// Liczy metryki dla pozycji manifestu (już przefiltrowanych po podziale) i ich wyników.
pub fn score(entries: &[ManifestEntry], results: &[ItemResult]) -> F2Report {
    let by_id: BTreeMap<&str, &ItemResult> = results.iter().map(|r| (r.id.as_str(), r)).collect();
    let mut rep = F2Report::default();
    for e in entries {
        let Some(r) = by_id.get(e.id.as_str()) else {
            rep.missing.push(e.id.clone());
            continue;
        };
        if e.kind.is_wer() {
            let (err, n) = word_errors(
                e.transcript.as_deref().unwrap_or_default(),
                r.hypothesis.as_deref().unwrap_or_default(),
            );
            rep.wer_errors += err;
            rep.wer_words += n;
        }
        match e.kind {
            ItemKind::Command
                if matches!(e.command, Some(CommandKind::Stop | CommandKind::Cancel)) =>
            {
                let hit = r.command == e.command;
                rep.stop_recall.add(hit);
                if let (true, Some(ms)) = (hit, r.reaction_ms) {
                    rep.stop_reactions_ms.push(ms);
                }
            }
            ItemKind::Backchannel => match r.interrupted {
                Some(false) => rep.backchannel_kept += 1,
                Some(true) => rep.backchannels_interrupting += 1,
                None => rep.missing.push(e.id.clone()),
            },
            ItemKind::Interruption => {
                if r.interrupted == Some(false) {
                    rep.interruptions_missed += 1;
                }
                if let (Some(truth), Some(got)) = (e.heard_words, r.heard_words) {
                    rep.prefix.add(truth.abs_diff(got) <= 1);
                }
                if let Some(intent) = e.intent {
                    rep.intents
                        .entry(intent_key(intent))
                        .or_default()
                        .add(r.intent == Some(intent));
                }
            }
            _ => {}
        }
    }
    rep
}

/// Fałszywe przerwania na godzinę z dziennika sesji (NDJSON zdarzeń magistrali; sesja bez mowy
/// właściciela): liczba `voice.dialog.interrupted` / czas sesji.
pub fn false_interruptions_per_hour(log_ndjson: &str) -> Result<f64, String> {
    let mut first = None;
    let mut last = None;
    let mut count = 0u64;
    for (i, line) in log_ndjson.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let e: core_bus_contract::Event =
            serde_json::from_str(line).map_err(|e| format!("dziennik, linia {}: {e}", i + 1))?;
        first.get_or_insert(e.ts);
        last = Some(e.ts);
        if e.kind.as_str() == voice_dialog_contract::EVENT_INTERRUPTED {
            count += 1;
        }
    }
    let (Some(a), Some(b)) = (first, last) else {
        return Err("pusty dziennik sesji".into());
    };
    let hours = (b - a).num_milliseconds() as f64 / 3_600_000.0;
    if hours <= 0.0 {
        return Err("dziennik sesji bez upływu czasu".into());
    }
    Ok(count as f64 / hours)
}
