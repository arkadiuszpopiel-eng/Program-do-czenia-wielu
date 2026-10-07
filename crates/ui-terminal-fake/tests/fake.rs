//! Atrapa przechodzi testy kontraktowe; `emit` i echo trafiają do odbiorcy.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use ui_terminal_contract::contract_tests::{RecordingSink, run_all};
use ui_terminal_contract::{OpenRequest, PtySize, TerminalProfile, TerminalService, ui_only};
use ui_terminal_fake::FakeTerminals;

#[test]
fn contract_and_echo() {
    let fake = FakeTerminals::new(2);
    run_all(&fake, 2);
    let sink = Arc::new(RecordingSink::default());
    let g = ui_only::user_gesture();
    let id = fake
        .open(
            OpenRequest {
                profile: TerminalProfile::ClaudeLogin,
                size: PtySize::default(),
                cwd: None,
            },
            &g,
            sink.clone(),
        )
        .unwrap();
    fake.emit(id, b"Zaloguj: ");
    fake.input(id, b"tak", &g).unwrap();
    assert_eq!(sink.output_of(id), b"Zaloguj: tak");
    assert_eq!(fake.opened().last(), Some(&TerminalProfile::ClaudeLogin));
}
