//! Opis luk po STOP (`kill_gap`): czego Sędzia Cieni i watchdog nie potwierdzili.
//! Testy poza `src` (limit rozmiaru crate'a).

use app_modules::broker::kill_gap;
use watchdog_contract::{JobFailure, KillReason, KillReport};

fn report(audited: bool, failed: usize) -> KillReport {
    KillReport {
        reason: KillReason::Hotkey,
        tokens_revoked: 2,
        jobs_killed: 1,
        jobs_failed: (0..failed)
            .map(|i| JobFailure {
                job: u32::try_from(i).unwrap_or(0),
                error: "odmowa dostępu".into(),
            })
            .collect(),
        audio_silenced: true,
        audited,
        latency_us: 10,
    }
}

#[test]
fn kill_gap_names_what_was_not_confirmed() {
    assert_eq!(kill_gap(&report(true, 0)), None);
    let broker = kill_gap(&report(false, 0)).unwrap_or_default();
    assert!(broker.contains("Sędzia Cieni nie potwierdził"), "{broker}");
    let jobs = kill_gap(&report(true, 2)).unwrap_or_default();
    assert_eq!(jobs, "nie udało się zatrzymać procesów: 2");
    let both = kill_gap(&report(false, 1)).unwrap_or_default();
    assert!(
        both.contains("Sędzia Cieni") && both.contains("procesów: 1"),
        "{both}"
    );
}
