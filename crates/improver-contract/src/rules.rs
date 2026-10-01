//! Reguły retrospektywy (deterministyczne, bez modelu): obserwacje → kandydaci na zmiany.

use serde_json::json;

use crate::guard::ChangeTarget;
use crate::proposal::{CandidateSet, MetricsSnapshot, Observation};

/// Minimalna liczba poprawek wymowy, by zaproponować wpis słownika.
pub const MIN_PRONUNCIATION_FIXES: u32 = 3;
/// Minimalna liczba prób trasy, by zaproponować wagę.
pub const MIN_ROUTE_SAMPLES: u32 = 20;
/// Minimalna liczba powtórzeń przepływu, by zaproponować umiejętność.
pub const MIN_FLOW_REPEATS: u32 = 5;

/// Segment klucza z dowolnego tekstu: `[a-z0-9_]`, polskie znaki zdjęte, max 40 znaków.
pub fn slug(text: &str) -> String {
    let fold = |c: char| match c {
        'ą' => 'a',
        'ć' => 'c',
        'ę' => 'e',
        'ł' => 'l',
        'ń' => 'n',
        'ó' => 'o',
        'ś' => 's',
        'ź' | 'ż' => 'z',
        c if c.is_ascii_alphanumeric() => c,
        _ => '_',
    };
    let s: String = text.to_lowercase().chars().map(fold).take(40).collect();
    let s = s.trim_matches('_').to_owned();
    if s.is_empty() { "x".into() } else { s }
}

fn round_to(v: f64, step: f64) -> f64 {
    (v / step).round() * step
}

/// Kandydaci z obserwacji.
pub fn propose(snapshot: &MetricsSnapshot) -> Vec<CandidateSet> {
    let mut out = Vec::new();
    for obs in &snapshot.observations {
        match obs {
            Observation::PronunciationFix {
                word,
                phonetic,
                count,
            } if *count >= MIN_PRONUNCIATION_FIXES => {
                out.push(CandidateSet {
                    title: format!("Wymowa słowa „{word}”"),
                    rationale: format!("Poprawiałeś wymowę {count} razy; dopisuję wpis słownika."),
                    source: "rule:pronunciation".into(),
                    targets: vec![ChangeTarget::Config {
                        key: format!("voice.persona.lexicon.{}", slug(word)),
                        value: json!(phonetic),
                    }],
                });
            }
            Observation::RouteOutcome {
                route,
                class,
                success_rate,
                n,
            } if *n >= MIN_ROUTE_SAMPLES && success_rate.is_finite() => {
                let weight = round_to(success_rate.clamp(0.0, 1.0), 0.05);
                out.push(CandidateSet {
                    title: format!("Waga trasy {route} dla „{class}”"),
                    rationale: format!("Skuteczność {:.0}% na {n} próbach.", success_rate * 100.0),
                    source: "rule:router-weights".into(),
                    targets: vec![ChangeTarget::Config {
                        key: format!("router.weights.{}_{}", slug(route), slug(class)),
                        value: json!(weight),
                    }],
                });
            }
            Observation::RepeatedFlow { name, steps, count }
                if *count >= MIN_FLOW_REPEATS && !steps.is_empty() =>
            {
                out.push(CandidateSet {
                    title: format!("Umiejętność „{name}”"),
                    rationale: format!("Ten przepływ powtórzył się {count} razy."),
                    source: "rule:repeated-flow".into(),
                    targets: vec![ChangeTarget::Config {
                        key: format!("skills.{}.playbook", slug(name)),
                        value: json!(
                            steps
                                .iter()
                                .enumerate()
                                .map(|(i, s)| format!("{}. {s}", i + 1))
                                .collect::<Vec<_>>()
                                .join("\n")
                        ),
                    }],
                });
            }
            Observation::FalseInterruptions { per_hour } if *per_hour > 1.0 => {
                out.push(CandidateSet {
                    title: "Mniej fałszywych przerwań".into(),
                    rationale: format!("{per_hour:.1} fałszywych przerwań na godzinę (próg 1/h)."),
                    source: "rule:false-interruptions".into(),
                    targets: vec![ChangeTarget::Config {
                        key: "voice.dialog.interrupt_min_ms".into(),
                        value: json!(450),
                    }],
                });
            }
            _ => {}
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rules_produce_allowed_candidates() {
        let snap = MetricsSnapshot {
            ts_ms: 0,
            metrics: Default::default(),
            observations: vec![
                Observation::PronunciationFix {
                    word: "Łódź".into(),
                    phonetic: "łucz".into(),
                    count: 3,
                },
                Observation::PronunciationFix {
                    word: "rzadko".into(),
                    phonetic: "x".into(),
                    count: 1,
                },
                Observation::RouteOutcome {
                    route: "local".into(),
                    class: "chat".into(),
                    success_rate: 0.83,
                    n: 40,
                },
                Observation::RepeatedFlow {
                    name: "Raport tygodniowy".into(),
                    steps: vec!["a".into()],
                    count: 6,
                },
                Observation::FalseInterruptions { per_hour: 2.5 },
            ],
        };
        let c = propose(&snap);
        assert_eq!(c.len(), 4);
        let keys: Vec<String> = c.iter().map(|s| s.targets[0].summary()).collect();
        assert_eq!(keys[0], "config:voice.persona.lexicon.lodz");
        assert_eq!(keys[1], "config:router.weights.local_chat");
        assert_eq!(keys[2], "config:skills.raport_tygodniowy.playbook");
        for set in &c {
            assert!(
                crate::guard::assess(&set.targets[0], None).is_ok(),
                "{set:?}"
            );
        }
        assert_eq!(slug("!!!"), "x");
    }
}
