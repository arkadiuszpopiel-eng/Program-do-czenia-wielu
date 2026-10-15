//! Prawdziwa sonda na tym komputerze (ręcznie: `cargo test -p platform-windows-diag-impl --test live -- --ignored --nocapture`).

#![allow(clippy::unwrap_used, clippy::expect_used)]

use pc_diag_contract::DiagProbe;

#[test]
#[ignore = "czyta prawdziwy system (kilka-kilkanaście sekund)"]
fn live_snapshot() {
    let start = std::time::Instant::now();
    let s = platform_windows_diag_impl::WinDiagProbe::default().snapshot().unwrap();
    println!("{} ms", start.elapsed().as_millis());
    println!("{}", pc_diag_contract::report::markdown(&s, None));
    pc_diag_contract::rating::integrity(&s).notes.iter().for_each(|n| println!("OCENA: {n}"));
}
