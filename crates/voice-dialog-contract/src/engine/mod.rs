//! Deterministyczny rdzeń automatu rozmowy (wspólny dla `voice-dialog-impl` i runtime potoku
//! głosu): `DialogMachine` (zatrzymanie dwustopniowe, backchannel, prefiks, intencje, wznawianie,
//! mowa proaktywna), heurystyczny klasyfikator intencji PL, usłyszany prefiks i sterownik z zasobem
//! głośnika. Ten sam wzorzec co `VadMachine`, `WakeMachine`, `LockTable` — SPEC wymaga, by
//! decyzje były czystą funkcją, a moduły zależą wyłącznie od kontraktów.

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

use crate::DialogConfig;

/// Automat z podaną konfiguracją i klasyfikatorem heurystycznym (frazy backchannelu z konfiguracji).
pub fn machine_with(cfg: DialogConfig) -> DialogMachine<HeuristicClassifier> {
    let classifier = HeuristicClassifier::new(cfg.backchannel_phrases.clone());
    DialogMachine::new(cfg, classifier)
}

/// Automat z konfiguracją domyślną i klasyfikatorem heurystycznym.
pub fn default_machine() -> DialogMachine<HeuristicClassifier> {
    machine_with(DialogConfig::default())
}
