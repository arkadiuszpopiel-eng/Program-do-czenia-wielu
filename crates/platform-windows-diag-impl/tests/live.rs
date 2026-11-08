//! Prawdziwa sonda na tym komputerze (ręcznie: `cargo test -p platform-windows-diag-impl --test live -- --ignored --nocapture`).

#![allow(clippy::unwrap_used, clippy::expect_used)]

use pc_diag_contract::DiagProbe;

#[test]
#[ignore = "czyta prawdziwy system (kilka-kilkanaście sekund)"]
fn live_snapshot() {
    let start = std::time::Instant::now();
    let s = platform_windows_diag_impl::WinDiagProbe::default()
        .snapshot()
        .unwrap();
    eprintln!("{} ms", start.elapsed().as_millis());
    eprintln!("{}", pc_diag_contract::report::markdown(&s, None));
    pc_diag_contract::rating::integrity(&s)
        .notes
        .iter()
        .for_each(|n| eprintln!("OCENA: {n}"));
}

#[test]
#[ignore = "czyta prawdziwy system (kilka sekund)"]
fn live_snapshot_stopped_after_limit_keeps_read_parts() {
    // Limit krótszy niż cała sonda: zapis powstaje z części odczytanych, reszta — luki (31.10).
    let s =
        platform_windows_diag_impl::WinDiagProbe::with_timeout(std::time::Duration::from_secs(3))
            .snapshot()
            .unwrap();
    eprintln!("luki: {:?}", s.gaps);
    assert!(
        s.gaps
            .iter()
            .any(|g| g.contains("nie zdążyło w limicie 3 s"))
    );
}
