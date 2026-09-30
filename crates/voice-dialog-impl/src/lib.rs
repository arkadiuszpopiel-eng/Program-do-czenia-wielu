//! Implementacja modułu `voice-dialog` (docs/modules/voice-dialog/SPEC.md): czysto funkcyjny automat
//! rozmowy `DialogMachine` (zatrzymanie dwustopniowe: ducking → twardy stop po potwierdzeniu,
//! backchannel nie przerywa, reguła „nie” przez `voice-cmd`, usłyszany prefiks ze znaczników słów lub
//! liczenia próbek, klasy intencji przerwania, wznawianie od punktu cięcia, mowa proaktywna z etykietą,
//! jedna agentka mówi naraz, przerwanie tekstem), heurystyczny klasyfikator intencji PL i sterownik
//! z zasobem głośnika.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

mod backchannel;
mod classifier;
mod driver;
mod machine;
mod prefix;

pub use backchannel::{BackchannelClass, classify as classify_backchannel};
pub use classifier::HeuristicClassifier;
pub use driver::DialogDriver;
pub use machine::DialogMachine;
pub use prefix::{heard_prefix, unsaid};

use voice_dialog_contract::DialogConfig;

/// Treść `module.toml` tego modułu.
pub const MODULE_TOML: &str = include_str!("../module.toml");

/// Automat z konfiguracją domyślną i klasyfikatorem heurystycznym.
pub fn default_machine() -> DialogMachine<HeuristicClassifier> {
    let cfg = DialogConfig::default();
    let classifier = HeuristicClassifier::new(cfg.backchannel_phrases.clone());
    DialogMachine::new(cfg, classifier)
}
