//! Implementacja modułu `voice-dialog` (docs/modules/voice-dialog/SPEC.md): manifest modułu i
//! re-eksport deterministycznego rdzenia z kontraktu — czysto funkcyjny automat rozmowy
//! `DialogMachine` (zatrzymanie dwustopniowe: ducking → twardy stop po potwierdzeniu,
//! backchannel nie przerywa, reguła „nie” przez `voice-cmd`, usłyszany prefiks ze znaczników słów
//! lub liczenia próbek, klasy intencji przerwania, wznawianie od punktu cięcia, mowa proaktywna
//! z etykietą, jedna agentka mówi naraz, przerwanie tekstem), heurystyczny klasyfikator intencji PL
//! i sterownik z zasobem głośnika. Rdzeń mieszka w `voice-dialog-contract` (wzorzec `VadMachine`),
//! żeby runtime potoku głosu mógł go złożyć bez zależności od `-impl`.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

pub use voice_dialog_contract::{
    BackchannelClass, DialogDriver, DialogMachine, HeuristicClassifier, classify_backchannel,
    default_machine, heard_prefix, machine_with, unsaid,
};

/// Treść `module.toml` tego modułu.
pub const MODULE_TOML: &str = include_str!("../module.toml");
