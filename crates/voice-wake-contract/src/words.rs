//! Słowa wywoławcze v1 (F5): walidacja konfiguracji fraz („Hej Alfa/Beta/Gama/Delta” i imiona
//! z Kreatora — `Persona::wake_phrases`), normalizacja fraz do porównań z etykietami modelu KWS
//! oraz strojenie detektora ([`KwsParams`]: próg z histerezą, okno odporności, limit ciszy).
//!
//! Domyślnie **wyłączone** (`WakeCfg::wake_words = None`); włączenie jest decyzją użytkownika
//! w Ustawieniach, rekomendowane dopiero po spełnieniu FAR ≤ 1/dzień i FRR ≤ 5% (F5-05/06).
//! Tryb „zawsze słucham” (`always_on`) jest poza v1 — wymaga weryfikacji właściciela na każdej
//! wypowiedzi w potoku, więc konfiguracja z nim jest odrzucana (`NotAvailable`).

use personas_contract::{Persona, PersonaId, fold};

use crate::{WakeError, WakeWordCfg};

/// Najmniej sylab we frazie (krótkie frazy = dużo fałszywych wybudzeń; SPEC: 3–4 sylaby).
pub const MIN_PHRASE_SYLLABLES: usize = 3;
/// Najwięcej znaków frazy.
pub const MAX_PHRASE_CHARS: usize = 40;
/// Najwięcej fraz.
pub const MAX_PHRASES: usize = 16;

/// Postać frazy do porównań: małe litery, bez polskich znaków i interpunkcji, pojedyncze spacje.
pub fn normalize_phrase(phrase: &str) -> String {
    fold(phrase)
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { ' ' })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Liczba sylab (grupy samogłosek PL; „ia/ie/io/iu” po spółgłosce liczone jako jedna).
pub fn syllables(phrase: &str) -> usize {
    let vowels = |c: char| matches!(c, 'a' | 'e' | 'i' | 'o' | 'u' | 'y');
    normalize_phrase(phrase)
        .split(' ')
        .map(|w| {
            let mut n = 0;
            let mut prev = false;
            for c in w.chars() {
                let v = vowels(c);
                if v && !prev {
                    n += 1;
                }
                prev = v;
            }
            n
        })
        .sum()
}

impl WakeWordCfg {
    /// Frazy z person (`wake_phrases`, np. „Hej Alfa”, imiona z Kreatora); `always_on` wyłączone,
    /// bramka właściciela włączona.
    pub fn from_personas(personas: &[Persona], threshold: f32) -> Self {
        Self {
            phrases: personas
                .iter()
                .flat_map(|p| p.wake_phrases.iter().map(|w| (w.clone(), p.id.clone())))
                .collect(),
            threshold,
            always_on: false,
            owner_gate: false,
        }
    }

    /// Walidacja: frazy (≥ 1, ≤ 16, różne, ≥ 3 sylaby, ≤ 40 znaków), próg w (0, 1),
    /// brak „zawsze słucham” (poza v1).
    pub fn validate(&self) -> Result<(), WakeError> {
        if self.phrases.is_empty() {
            return Err(WakeError::NotAvailable(
                "słowa wywoławcze: brak fraz (dodaj imiona w Kreatorze)".into(),
            ));
        }
        if self.always_on {
            return Err(WakeError::NotAvailable(
                "tryb „zawsze słucham” wymaga weryfikacji właściciela na każdej wypowiedzi — poza v1"
                    .into(),
            ));
        }
        if !(self.threshold > 0.0 && self.threshold < 1.0) {
            return Err(WakeError::InvalidConfig(
                "próg słów wywoławczych musi być w (0, 1)".into(),
            ));
        }
        if self.phrases.len() > MAX_PHRASES {
            return Err(WakeError::InvalidConfig(format!(
                "najwyżej {MAX_PHRASES} fraz wywoławczych"
            )));
        }
        let mut seen: Vec<String> = Vec::new();
        for (phrase, _) in &self.phrases {
            let norm = normalize_phrase(phrase);
            if norm.is_empty() || phrase.chars().count() > MAX_PHRASE_CHARS {
                return Err(WakeError::InvalidConfig(format!(
                    "fraza „{phrase}”: 1–{MAX_PHRASE_CHARS} znaków"
                )));
            }
            if syllables(&norm) < MIN_PHRASE_SYLLABLES {
                return Err(WakeError::InvalidConfig(format!(
                    "fraza „{phrase}” jest za krótka (≥ {MIN_PHRASE_SYLLABLES} sylaby) — \
                     krótkie frazy często budzą się same"
                )));
            }
            if seen.contains(&norm) {
                return Err(WakeError::InvalidConfig(format!(
                    "fraza „{phrase}” powtórzona"
                )));
            }
            seen.push(norm);
        }
        Ok(())
    }

    /// Persona dla etykiety modelu (porównanie po [`normalize_phrase`]).
    pub fn persona_for(&self, label: &str) -> Option<(&str, &PersonaId)> {
        let want = normalize_phrase(label);
        self.phrases
            .iter()
            .find(|(p, _)| normalize_phrase(p) == want)
            .map(|(p, id)| (p.as_str(), id))
    }
}

/// Strojenie detektora słów wywoławczych (wartości startowe — do strojenia na korpusie, F5-05/06).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct KwsParams {
    /// Histereza: ponowne uzbrojenie dopiero po spadku wyniku poniżej `próg − histereza`.
    pub hysteresis: f32,
    /// Ile kolejnych kroków modelu ≥ próg, zanim padnie wykrycie (odporność na pojedyncze piki).
    pub min_hits: u32,
    /// Okno odporności po wykryciu (ms) — kolejne wykrycia ignorowane.
    pub refractory_ms: u64,
    /// Ile ms po wybudzeniu czekać na mowę; brak mowy = podejrzenie fałszywego alarmu.
    pub listen_timeout_ms: u64,
    /// Bufor pierścieniowy audio (ms) — jedyne miejsce, gdzie audio leży przed wykryciem.
    pub ring_ms: u32,
    /// Bramka energii/VAD przed modelem (oszczędza CPU i tnie fałszywe alarmy na ciszy).
    pub vad_gate: bool,
    /// Ile ms audio sprzed otwarcia bramki podać modelowi (początek „Hej”).
    pub gate_preroll_ms: u32,
    /// Ile ms bramka trzyma się otwarta po ostatniej ramce mowy.
    pub gate_hang_ms: u32,
}

impl Default for KwsParams {
    fn default() -> Self {
        Self {
            hysteresis: 0.15,
            min_hits: 2,
            refractory_ms: 2_000,
            listen_timeout_ms: 6_000,
            ring_ms: 2_000,
            vad_gate: true,
            gate_preroll_ms: 400,
            gate_hang_ms: 600,
        }
    }
}

impl KwsParams {
    /// Walidacja.
    pub fn validate(&self) -> Result<(), WakeError> {
        let bad = |m: &str| Err(WakeError::InvalidConfig(format!("KWS: {m}")));
        if !(0.0..0.5).contains(&self.hysteresis) {
            return bad("histereza 0…0,5");
        }
        if self.min_hits == 0 || self.min_hits > 20 {
            return bad("min_hits 1…20");
        }
        if !(500..=4_000).contains(&self.ring_ms) {
            return bad("bufor pierścieniowy 0,5–4 s");
        }
        if self.gate_preroll_ms > self.ring_ms {
            return bad("pre-roll bramki dłuższy niż bufor");
        }
        if self.listen_timeout_ms < 1_000 {
            return bad("limit ciszy po wybudzeniu ≥ 1 s");
        }
        Ok(())
    }
}
