//! Dziennik zdarzeń IPC (`app_api::event_log`): łańcuch ramek, numery, wznowienie z luką,
//! wypadanie starych ramek z bufora.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use app_api::dto::AlfaEvent;
use app_api::event_log::*;

fn event(id: &str) -> AlfaEvent {
    AlfaEvent::SessionRemoved {
        session_id: id.into(),
    }
}

#[test]
fn frames_chain_by_prev_and_seq() {
    let log = EventLog::new();
    let a = log.seal(3, vec![event("a")]);
    let b = log.seal(7, vec![event("b"), event("c")]);
    assert_eq!((a.prev, a.seq, a.v), (0, 3, FRAME_VERSION));
    assert_eq!((b.prev, b.seq), (3, 7));
    assert_eq!(a.epoch, log.epoch());
    assert_eq!(log.latest(), 7);
    assert_eq!(b.len(), 2, "ramka odwołuje się jak lista zdarzeń");
}

#[test]
fn issue_numbers_grow_by_one() {
    let log = EventLog::new();
    let mut seen = Vec::new();
    for _ in 0..3 {
        log.issue(|seq| seen.push(seq));
    }
    assert_eq!(seen, [1, 2, 3]);
    assert_eq!(log.issued(), 3);
}

#[test]
fn resume_returns_missing_frames() {
    let log = EventLog::new();
    log.seal(2, vec![event("a")]);
    log.seal(5, vec![event("b")]);
    log.seal(6, vec![event("c")]);
    let all = log.resume(0, Some(log.epoch()));
    assert!(all.complete);
    assert_eq!(
        all.frames.iter().map(|f| f.seq).collect::<Vec<_>>(),
        [2, 5, 6]
    );
    let tail = log.resume(2, None);
    assert!(tail.complete);
    assert_eq!(
        tail.frames.iter().map(|f| f.seq).collect::<Vec<_>>(),
        [5, 6]
    );
    assert_eq!(tail.latest_seq, 6);
    let none = log.resume(6, None);
    assert!(none.complete && none.frames.is_empty());
}

#[test]
fn resume_reports_gap_instead_of_guessing() {
    let log = EventLog::new();
    log.seal(2, vec![event("a")]);
    log.seal(5, vec![event("b")]);
    // Numer w środku ramki, z przyszłości i z innego uruchomienia.
    for (after, epoch) in [(3, None), (9, None), (2, Some("inne"))] {
        let r = log.resume(after, epoch);
        assert!(!r.complete, "after={after} epoch={epoch:?}");
        assert!(r.frames.is_empty());
        assert_eq!(r.latest_seq, 5);
        assert_eq!(r.epoch, log.epoch());
    }
    // Pusty bufor: klient bez historii jest na bieżąco.
    assert!(EventLog::new().resume(0, None).complete);
}

#[test]
fn old_frames_fall_out_of_the_buffer() {
    let log = EventLog::new();
    let total = LOG_MAX_FRAMES as u64 + 10;
    for seq in 1..=total {
        log.seal(seq, vec![event("x")]);
    }
    assert!(!log.resume(0, None).complete, "początek wypadł z bufora");
    let recent = log.resume(total - 5, None);
    assert!(recent.complete);
    assert_eq!(recent.frames.len(), 5);
    // Budżet zdarzeń: jedna wielka ramka wypiera starsze, ale sama zostaje.
    let big = EventLog::new();
    big.seal(1, vec![event("a")]);
    big.seal(2, (0..=LOG_MAX_EVENTS).map(|_| event("b")).collect());
    assert!(!big.resume(0, None).complete);
    assert_eq!(big.resume(1, None).frames.len(), 1);
}
