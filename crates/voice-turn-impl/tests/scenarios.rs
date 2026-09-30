//! Scenariusze z wirtualnym zegarem: kontrakt, pytania, hezytacje, model, limity, cierpliwość.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use core_registry_contract::ModuleManifest;
use voice_turn_contract::contract_tests::{self, poll_end};
use voice_turn_contract::{
    EndReason, Patience, TurnCfg, TurnDecision, TurnDetector, TurnError, TurnEvent, WaitReason,
};
use voice_turn_fake::ScriptedTurnModel;
use voice_turn_impl::{HeuristicTurnModel, MODULE_TOML, PatienceTurnDetector};

/// Jedna tura: mowa 1000–`end`, transkrypt `text`; zwraca czas końca tury (odpytywanie co 10 ms).
fn turn<D: TurnDetector>(d: &mut D, text: &str, end: u64) -> u64 {
    d.observe(&TurnEvent::SpeechStart { at_ms: 1000 });
    d.observe(&TurnEvent::Partial {
        at_ms: end - 100,
        text: text.into(),
    });
    d.observe(&TurnEvent::SpeechEnd { at_ms: end });
    poll_end(d, end, end + 5000, 10).unwrap() - end
}

fn heuristic() -> PatienceTurnDetector<HeuristicTurnModel> {
    PatienceTurnDetector::new(HeuristicTurnModel)
}

#[test]
fn contract_suite_heuristic_and_scripted_model() {
    contract_tests::run_all(heuristic);
    contract_tests::run_all(|| PatienceTurnDetector::new(ScriptedTurnModel::constant(0.6)));
}

#[test]
fn manifest_is_valid() {
    let m = ModuleManifest::parse_toml(MODULE_TOML).unwrap();
    assert_eq!(m.id.as_str(), "voice-turn");
    assert_eq!(m.provides[0].to_string(), "voice-turn-contract@1");
}

#[test]
fn clear_question_ends_fast_within_stage_budget() {
    let silence = turn(&mut heuristic(), "Jaka jest jutro pogoda?", 2500);
    assert_eq!(silence, 200);
    // Budżet etapu końca tury (§6.4): 250–500 ms razem z ramką VAD — tu ≤ 500 ms.
    assert!(silence <= 500);
    assert_eq!(turn(&mut heuristic(), "Otwórz pocztę.", 2500), 200);
}

#[test]
fn hesitations_extend_patience() {
    let normal = Patience::Normal.cfg();
    let filler = turn(&mut heuristic(), "Chciałabym, żeby yyy", 3000);
    let unfinished = turn(&mut heuristic(), "kupiłam mleko i", 3000);
    let plain = turn(&mut heuristic(), "otwórz pocztę", 3000);
    assert_eq!(plain, normal.base_ms);
    assert_eq!(
        filler,
        normal.base_ms + normal.low_prob_bonus_ms + normal.hesitation_bonus_ms
    );
    assert_eq!(unfinished, filler);
    assert!(filler <= normal.max_ms);
}

#[test]
fn hesitation_wait_reports_reason_and_resume_cancels() {
    let mut d = heuristic();
    d.observe(&TurnEvent::SpeechStart { at_ms: 0 });
    d.observe(&TurnEvent::Partial {
        at_ms: 400,
        text: "to znaczy".into(),
    });
    d.observe(&TurnEvent::SpeechEnd { at_ms: 500 });
    assert!(matches!(
        d.decide(600, None),
        TurnDecision::Wait {
            reason: WaitReason::Hesitation,
            ..
        }
    ));
    d.observe(&TurnEvent::SpeechStart { at_ms: 900 });
    d.observe(&TurnEvent::Partial {
        at_ms: 1400,
        text: "to znaczy jutro rano.".into(),
    });
    d.observe(&TurnEvent::SpeechEnd { at_ms: 1500 });
    assert_eq!(poll_end(&mut d, 1500, 4000, 10), Some(1700));
}

#[test]
fn model_scores_drive_required_silence() {
    let cfg = Patience::Normal.cfg();
    let confident = turn(
        &mut PatienceTurnDetector::new(ScriptedTurnModel::constant(0.95)),
        "coś",
        2000,
    );
    assert_eq!(confident, cfg.min_silence_ms);
    let unsure = turn(
        &mut PatienceTurnDetector::new(ScriptedTurnModel::constant(0.1)),
        "coś",
        2000,
    );
    assert_eq!(unsure, cfg.base_ms + cfg.low_prob_bonus_ms);
    let model = ScriptedTurnModel::constant(0.6);
    model.push(Err(TurnError::Model {
        reason: "brak pliku ONNX".into(),
    }));
    // Błąd modelu → sama polityka tekstowa (kropka = wyraźny koniec).
    assert_eq!(
        turn(&mut PatienceTurnDetector::new(model), "Dziękuję.", 2000),
        cfg.min_silence_ms
    );
}

#[test]
fn model_is_called_once_per_speech_end_and_text_version() {
    let mut d = PatienceTurnDetector::new(ScriptedTurnModel::constant(0.6));
    turn(&mut d, "Hej.", 2000);
    assert_eq!(d.model().calls().len(), 1);
}

#[test]
fn max_silence_is_a_hard_cap() {
    let custom = voice_turn_contract::PatienceCfg {
        min_silence_ms: 200,
        base_ms: 300,
        hesitation_bonus_ms: 5000,
        low_prob_bonus_ms: 5000,
        max_ms: 900,
    };
    let cfg = TurnCfg {
        patience: Patience::Custom(custom),
        ..TurnCfg::default()
    };
    let mut d = PatienceTurnDetector::with_cfg(ScriptedTurnModel::constant(0.01), cfg).unwrap();
    d.observe(&TurnEvent::SpeechStart { at_ms: 0 });
    d.observe(&TurnEvent::Partial {
        at_ms: 100,
        text: "i yyy".into(),
    });
    d.observe(&TurnEvent::SpeechEnd { at_ms: 1000 });
    let mut end = None;
    for t in (1000..3000).step_by(10) {
        if let TurnDecision::EndOfTurn { at_ms, reason, .. } = d.decide(t, None) {
            end = Some((at_ms, reason));
            break;
        }
    }
    assert_eq!(end, Some((1900, EndReason::MaxSilence)));
}

#[test]
fn patience_levels_order_and_partial_text_toggle() {
    let with = |p: Patience, text: &str| {
        let cfg = TurnCfg {
            patience: p,
            ..TurnCfg::default()
        };
        turn(
            &mut PatienceTurnDetector::with_cfg(HeuristicTurnModel, cfg).unwrap(),
            text,
            2000,
        )
    };
    assert!(with(Patience::Low, "no i") < with(Patience::Normal, "no i"));
    assert!(with(Patience::Normal, "no i") < with(Patience::High, "no i"));
    let off = TurnCfg {
        use_partial_text: false,
        ..TurnCfg::default()
    };
    let silence = turn(
        &mut PatienceTurnDetector::with_cfg(HeuristicTurnModel, off).unwrap(),
        "no i",
        2000,
    );
    assert_eq!(silence, Patience::Normal.cfg().base_ms);
    assert!(
        PatienceTurnDetector::with_cfg(
            HeuristicTurnModel,
            TurnCfg {
                eot_threshold: 0.0,
                ..TurnCfg::default()
            }
        )
        .is_err()
    );
}
