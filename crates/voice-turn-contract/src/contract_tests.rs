//! Współdzielony test kontraktowy `TurnDetector` (feature `contract-tests`).

use crate::{Patience, TurnCfg, TurnDecision, TurnDetector, TurnEvent, WaitReason};

/// Odpytuje detektor co `step` ms od `from` do `to`; zwraca pierwszą decyzję `EndOfTurn` (czas).
pub fn poll_end<D: TurnDetector>(d: &mut D, from: u64, to: u64, step: u64) -> Option<u64> {
    let mut t = from;
    while t <= to {
        if let TurnDecision::EndOfTurn { at_ms, .. } = d.decide(t, None) {
            return Some(at_ms);
        }
        t += step.max(1);
    }
    None
}

/// Przed mową i po `Reset` — `Idle`; w trakcie mowy — `Wait(UserSpeaking)`.
pub fn idle_and_speaking<D: TurnDetector>(d: &mut D) {
    assert_eq!(d.decide(0, None), TurnDecision::Idle);
    d.observe(&TurnEvent::SpeechStart { at_ms: 100 });
    assert!(matches!(
        d.decide(150, None),
        TurnDecision::Wait {
            reason: WaitReason::UserSpeaking,
            ..
        }
    ));
    d.observe(&TurnEvent::Reset);
    assert_eq!(d.decide(200, None), TurnDecision::Idle);
}

/// Koniec tury nie wcześniej niż minimalna cisza i nie później niż `max_ms`; tylko raz na turę.
pub fn end_within_bounds_once<D: TurnDetector>(d: &mut D) {
    let p = d.config().patience.cfg();
    d.observe(&TurnEvent::SpeechStart { at_ms: 1000 });
    d.observe(&TurnEvent::Partial {
        at_ms: 1500,
        text: "Jaka jest jutro pogoda w Krakowie?".into(),
    });
    d.observe(&TurnEvent::SpeechEnd { at_ms: 2000 });
    let end =
        poll_end(d, 2000, 2000 + p.max_ms + 50, 10).unwrap_or_else(|| panic!("brak końca tury"));
    assert!(
        end >= 2000 + p.min_silence_ms && end <= 2000 + p.max_ms,
        "koniec o {end}"
    );
    assert_eq!(d.decide(end + 10, None), TurnDecision::Idle);
}

/// Wznowienie mowy przed końcem ciszy anuluje koniec tury.
pub fn resumed_speech_cancels_end<D: TurnDetector>(d: &mut D) {
    let p = d.config().patience.cfg();
    d.observe(&TurnEvent::SpeechStart { at_ms: 0 });
    d.observe(&TurnEvent::SpeechEnd { at_ms: 500 });
    assert!(poll_end(d, 500, 500 + p.min_silence_ms - 10, 10).is_none());
    d.observe(&TurnEvent::SpeechStart {
        at_ms: 500 + p.min_silence_ms - 5,
    });
    assert!(matches!(
        d.decide(500 + p.max_ms + 100, None),
        TurnDecision::Wait {
            reason: WaitReason::UserSpeaking,
            ..
        }
    ));
    d.observe(&TurnEvent::SpeechEnd { at_ms: 3000 });
    let end = poll_end(d, 3000, 3000 + p.max_ms, 10).unwrap_or_else(|| panic!("brak końca"));
    assert!(end >= 3000 + p.min_silence_ms);
}

/// `configure` waliduje i zmienia cierpliwość.
pub fn configure_validates<D: TurnDetector>(d: &mut D) {
    let high = TurnCfg {
        patience: Patience::High,
        ..TurnCfg::default()
    };
    d.configure(high).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(d.config().patience, Patience::High);
    let bad = TurnCfg {
        eot_threshold: 2.0,
        ..TurnCfg::default()
    };
    assert!(d.configure(bad).is_err());
    assert_eq!(d.config().patience, Patience::High);
}

/// Uruchamia zestaw; `factory` daje świeży detektor z konfiguracją domyślną.
pub fn run_all<D, F>(factory: F)
where
    D: TurnDetector,
    F: Fn() -> D,
{
    idle_and_speaking(&mut factory());
    end_within_bounds_once(&mut factory());
    resumed_speech_cancels_end(&mut factory());
    configure_validates(&mut factory());
}
