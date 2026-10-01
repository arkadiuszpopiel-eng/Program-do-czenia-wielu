//! Raport F2 z progami z docs/ACCEPTANCE.md §5 (wartości tylko do odczytu — zmiana progów wymaga
//! zatwierdzenia człowieka).

use crate::eval::metrics::{F2Report, percentile};

/// WER PL (F2-03).
pub const MAX_WER: f64 = 0.12;
/// Recall „stop/anuluj” (F2-04).
pub const MIN_STOP_RECALL: f64 = 0.99;
/// Reakcja na „stop/anuluj” (F2-04), ms.
pub const MAX_STOP_REACTION_MS: u64 = 300;
/// Precision backchannelu (F2-05).
pub const MIN_BACKCHANNEL_PRECISION: f64 = 0.95;
/// Fałszywe przerwania na godzinę (F2-06).
pub const MAX_FALSE_PER_HOUR: f64 = 1.0;
/// Prefiks ±1 słowo (F2-07).
pub const MIN_PREFIX_ACCURACY: f64 = 0.90;
/// Intencje per klasa (F2-08).
pub const MIN_INTENT_ACCURACY: f64 = 0.90;

/// Werdykt jednego kryterium (`pass = None` — brak danych).
#[derive(Debug, Clone, PartialEq)]
pub struct Verdict {
    /// Identyfikator kryterium.
    pub id: String,
    /// Opis.
    pub metric: String,
    /// Wartość.
    pub value: String,
    /// Próg.
    pub threshold: String,
    /// Wynik.
    pub pass: Option<bool>,
}

fn v(
    id: &str,
    metric: &str,
    value: Option<f64>,
    fmt: impl Fn(f64) -> String,
    threshold: &str,
    ok: impl Fn(f64) -> bool,
) -> Verdict {
    Verdict {
        id: id.into(),
        metric: metric.into(),
        value: value.map_or_else(|| "brak danych".into(), &fmt),
        threshold: threshold.into(),
        pass: value.map(ok),
    }
}

fn pct(x: f64) -> String {
    format!("{:.1} %", x * 100.0)
}

/// Werdykty kryteriów F2-03 … F2-08.
pub fn verdicts(r: &F2Report) -> Vec<Verdict> {
    let mut out = vec![
        v("F2-03", "WER PL", r.wer(), pct, "≤ 12 %", |x| {
            x <= MAX_WER
        }),
        v(
            "F2-04",
            "recall „stop/anuluj”",
            r.stop_recall.value(),
            pct,
            "≥ 99 %",
            |x| x >= MIN_STOP_RECALL,
        ),
    ];
    let under = (!r.stop_reactions_ms.is_empty()).then(|| {
        r.stop_reactions_ms
            .iter()
            .filter(|ms| **ms < MAX_STOP_REACTION_MS)
            .count() as f64
            / r.stop_reactions_ms.len() as f64
    });
    let p95 =
        percentile(&r.stop_reactions_ms, 95.0).map_or_else(|| "—".into(), |p| format!("{p} ms"));
    out.push(v(
        "F2-04",
        &format!("reakcja < 300 ms (p95 {p95})"),
        under,
        pct,
        "100 % prób < 300 ms",
        |x| x >= 1.0,
    ));
    out.push(v(
        "F2-05",
        "precision backchannelu",
        r.backchannel_precision(),
        pct,
        "≥ 95 %",
        |x| x >= MIN_BACKCHANNEL_PRECISION,
    ));
    out.push(v(
        "F2-06",
        "fałszywe przerwania",
        r.false_per_hour,
        |x| format!("{x:.2} / h"),
        "≤ 1 / h",
        |x| x <= MAX_FALSE_PER_HOUR,
    ));
    out.push(v(
        "F2-07",
        "prefiks ±1 słowo",
        r.prefix.value(),
        pct,
        "≥ 90 %",
        |x| x >= MIN_PREFIX_ACCURACY,
    ));
    for (intent, ratio) in &r.intents {
        out.push(v(
            "F2-08",
            &format!("intencja `{intent}` (n = {})", ratio.total),
            ratio.value(),
            pct,
            "≥ 90 %",
            |x| x >= MIN_INTENT_ACCURACY,
        ));
    }
    out
}

/// Raport Markdown (do `evals/spikes/…/results/` albo artefaktu CI self-hosted).
pub fn to_markdown(r: &F2Report) -> String {
    let mut s =
        String::from("| Kryterium | Metryka | Wartość | Próg | Wynik |\n|---|---|---|---|---|\n");
    for x in verdicts(r) {
        let pass = match x.pass {
            Some(true) => "✅",
            Some(false) => "❌",
            None => "—",
        };
        s.push_str(&format!(
            "| {} | {} | {} | {} | {pass} |\n",
            x.id, x.metric, x.value, x.threshold
        ));
    }
    if !r.missing.is_empty() {
        s.push_str(&format!(
            "\nBez wyniku: {} pozycji ({}).\n",
            r.missing.len(),
            r.missing.join(", ")
        ));
    }
    s
}
