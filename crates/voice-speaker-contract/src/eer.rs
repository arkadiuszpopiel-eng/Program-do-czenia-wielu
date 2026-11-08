//! Metryki weryfikacji mówcy (ACCEPTANCE F5-07/F5-08): EER, FAR/FRR przy progu, próg dla
//! zadanego FAR (akcje ryzykowne: FAR ≤ 0,1% przy ≥ 3000 prób obcych), punkty DET.
//! Akceptacja: wynik ≥ próg. Deterministyczne (sortowanie + wyszukiwanie binarne).

use serde::{Deserialize, Serialize};

/// Próg F5-07: EER.
pub const EER_MAX: f64 = 0.03;
/// Próg F5-08: FAR przy progu dla akcji ryzykownych.
pub const FAR_STRICT_MAX: f64 = 0.001;
/// Minimum prób obcych dla wiarygodnego FAR ≤ 0,1%.
pub const MIN_IMPOSTOR_TRIALS: usize = 3_000;

/// Błędy przy progu.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ErrorRates {
    /// Próg (akceptacja: wynik ≥ próg).
    pub threshold: f32,
    /// Odsetek obcych przyjętych.
    pub far: f64,
    /// Odsetek właściciela odrzuconego.
    pub frr: f64,
}

/// Raport EER.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EerReport {
    /// Próby właściciela.
    pub genuine: usize,
    /// Próby obcych.
    pub impostor: usize,
    /// EER.
    pub eer: f64,
    /// Próg w punkcie EER.
    pub eer_threshold: f32,
    /// Najniższy próg z FAR ≤ 0,1% i FRR przy nim.
    pub strict: Option<ErrorRates>,
    /// Błędy przy progach z konfiguracji (standardowy, ścisły).
    pub at_config: Vec<ErrorRates>,
    /// F5-07: EER ≤ 3%.
    pub f5_07_ok: bool,
    /// F5-08: FAR ≤ 0,1% przy progu ścisłym z konfiguracji.
    pub f5_08_ok: bool,
    /// ≥ 3000 prób obcych (inaczej F5-08 niewiarygodne).
    pub sufficient_impostors: bool,
}

fn sorted(v: &[f32]) -> Vec<f32> {
    let mut s: Vec<f32> = v.iter().copied().filter(|x| x.is_finite()).collect();
    s.sort_by(f32::total_cmp);
    s
}

fn rates_sorted(gen_sorted: &[f32], imp_sorted: &[f32], t: f32) -> ErrorRates {
    let rejected = gen_sorted.partition_point(|x| *x < t);
    let accepted = imp_sorted.len() - imp_sorted.partition_point(|x| *x < t);
    let ratio = |n: usize, d: usize| if d == 0 { 0.0 } else { n as f64 / d as f64 };
    ErrorRates {
        threshold: t,
        far: ratio(accepted, imp_sorted.len()),
        frr: ratio(rejected, gen_sorted.len()),
    }
}

/// FAR i FRR przy progu.
pub fn rates_at(genuine: &[f32], impostor: &[f32], threshold: f32) -> ErrorRates {
    rates_sorted(&sorted(genuine), &sorted(impostor), threshold)
}

fn candidates(g: &[f32], i: &[f32]) -> Vec<f32> {
    let mut c: Vec<f32> = g.iter().chain(i).copied().collect();
    c.sort_by(f32::total_cmp);
    c.dedup();
    if let Some(max) = c.last().copied() {
        c.push(max + 1e-4);
    }
    c
}

/// EER i próg w jego punkcie (`None` bez prób obu rodzajów).
pub fn eer(genuine: &[f32], impostor: &[f32]) -> Option<(f64, f32)> {
    let (g, i) = (sorted(genuine), sorted(impostor));
    if g.is_empty() || i.is_empty() {
        return None;
    }
    candidates(&g, &i)
        .into_iter()
        .map(|t| rates_sorted(&g, &i, t))
        .min_by(|a, b| (a.far - a.frr).abs().total_cmp(&(b.far - b.frr).abs()))
        .map(|r| ((r.far + r.frr) / 2.0, r.threshold))
}

/// Najniższy próg z FAR ≤ `target` i błędy przy nim.
pub fn threshold_for_far(genuine: &[f32], impostor: &[f32], target: f64) -> Option<ErrorRates> {
    let (g, i) = (sorted(genuine), sorted(impostor));
    if i.is_empty() {
        return None;
    }
    candidates(&g, &i)
        .into_iter()
        .map(|t| rates_sorted(&g, &i, t))
        .find(|r| r.far <= target)
}

/// Pełny raport dla progów konfiguracji `(standardowy, ścisły)`.
pub fn report(genuine: &[f32], impostor: &[f32], standard: f32, strict: f32) -> EerReport {
    let (g, i) = (sorted(genuine), sorted(impostor));
    let (eer_value, eer_threshold) = eer(&g, &i).unwrap_or((1.0, 0.0));
    let at_config = vec![rates_sorted(&g, &i, standard), rates_sorted(&g, &i, strict)];
    EerReport {
        genuine: g.len(),
        impostor: i.len(),
        eer: eer_value,
        eer_threshold,
        strict: threshold_for_far(&g, &i, FAR_STRICT_MAX),
        f5_07_ok: !g.is_empty() && !i.is_empty() && eer_value <= EER_MAX,
        f5_08_ok: !i.is_empty() && at_config[1].far <= FAR_STRICT_MAX,
        sufficient_impostors: i.len() >= MIN_IMPOSTOR_TRIALS,
        at_config,
    }
}
