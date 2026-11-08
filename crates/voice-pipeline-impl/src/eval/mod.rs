//! Zestaw dev/test pod ACCEPTANCE F2 (`evals/F2/`): manifest NDJSON (plik audio, transkrypt,
//! warunki, podział dev/test, etykiety), walidacja formatu, runner offline na kontraktach,
//! metryki (WER PL, recall „stop/anuluj” z reakcją, precision backchannelu, fałszywe przerwania/h,
//! prefiks ±1 słowo, intencje per klasa), zamrożenie podziału test (SHA-256) i raport z progami.
//! CLI: `alfa-voice-eval`; prawdziwy STT przez `whisper-cli` — [`PrefixStt`] (partial = model na
//! audio do teraz, jak w potoku) albo szybki [`TimedStt`] (jedna oś słów na pozycję).

pub mod audio;
pub mod freeze;
pub mod manifest;
pub mod metrics;
pub mod prefix;
pub mod report;
pub mod results;
pub mod runner;
pub mod timed;

pub use freeze::{freeze_list, verify_frozen};
pub use manifest::{
    ItemKind, ManifestEntry, Split, manifest_schema, parse_manifest, validate_manifest,
};
pub use metrics::{F2Report, Ratio, false_interruptions_per_hour, score, word_errors};
pub use prefix::{PrefixStt, WhisperCli};
pub use report::{Verdict, to_markdown, verdicts};
pub use results::{ItemResult, parse_results, results_schema, to_ndjson};
pub use runner::{Engines, run_item};
pub use timed::{TimedStt, parse_whisper_words, whisper_words};

/// Pozycje wybranego podziału (`None` = wszystkie).
pub fn select(entries: &[ManifestEntry], split: Option<Split>) -> Vec<ManifestEntry> {
    entries
        .iter()
        .filter(|e| split.is_none_or(|s| e.split == s))
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests;
