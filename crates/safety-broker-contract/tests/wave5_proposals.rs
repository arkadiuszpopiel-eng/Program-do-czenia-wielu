//! Fala 5 — **propozycje** dla Jądra (Broker), do decyzji człowieka; testy `#[ignore]` opisują
//! oczekiwane zachowanie i dziś padają (uruchomienie: `cargo test -p safety-broker-contract
//! --test wave5_proposals -- --ignored`).
//!
//! PT-25 (`evals/F9/pentest.md`): menedżery haseł i okno poświadczeń Windows są dziś chronione
//! tylko przez strażnika celów platformy (`platform_contract::SENSITIVE_APPS` → `TargetGuard`,
//! fala 5) i maskowanie zrzutów. Propozycja obrony w głąb: Broker odmawia **wydania tokenu**
//! `gui.control` dla tych aplikacji twardą regułą `CredentialDenylist` (na każdym poziomie, także
//! L4), z listą bazową jak `PROTECTED_PROCESSES` (nadzbiór `SENSITIVE_APPS`, alias 8.3).

#![allow(clippy::unwrap_used, clippy::expect_used)]

use compliance_contract::PathEnv;
use safety_broker_contract::{
    AppSelector, Capability, DeclaredFacts, KernelGuard, KernelPolicy, KernelRule,
};

/// Lista z `platform_contract::SENSITIVE_APPS` (crate kontraktu Brokera nie zależy od platformy —
/// zgodność pilnowałby test jak dla `PROTECTED_PROCESSES` w `tests/review.rs`).
const SENSITIVE_APPS: [&str; 8] = [
    "keepass.exe",
    "keepassxc.exe",
    "1password.exe",
    "bitwarden.exe",
    "dashlane.exe",
    "enpass.exe",
    "credentialuibroker.exe",
    "consent.exe",
];

fn guard() -> KernelGuard {
    let env = PathEnv::windows_profile(r"C:\Users\ala");
    let p = KernelPolicy::baseline(r"C:\Users\ala", r"C:\ProgramData\AlfaBroker").unwrap();
    KernelGuard::new(p, env)
}

#[test]
#[ignore = "propozycja PT-25 dla Brokera (Jądro) — wymaga decyzji i przeglądu człowieka"]
fn gui_control_of_password_managers_is_a_kernel_block() {
    let g = guard();
    for app in SENSITIVE_APPS.into_iter().chain([
        "KeePass",
        r"C:\Program Files\KeePassXC\KeePassXC.exe",
        "KEEPAS~1.EXE",
    ]) {
        let cap = Capability::GuiControl(AppSelector::parse(app).unwrap());
        assert_eq!(
            g.check_request(&cap, &DeclaredFacts::new("t")),
            Some(KernelRule::CredentialDenylist),
            "{app}"
        );
    }
    let word = Capability::GuiControl(AppSelector::parse("winword.exe").unwrap());
    assert_eq!(g.check_request(&word, &DeclaredFacts::new("t")), None);
}
