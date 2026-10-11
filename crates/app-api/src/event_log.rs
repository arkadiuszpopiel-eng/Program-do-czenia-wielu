//! Numery kolejne zdarzeń i wznowienie „od `seq`” (plan v6, zadanie B.2; ADR 0019).
//!
//! Każde zdarzenie dostaje numer w chwili wysłania (`EventHub::emit`). Paczka wychodzi jako ramka
//! [`EventFrame`]: `seq` — numer ostatniego zdarzenia w ramce, `prev` — `seq` ramki poprzedniej.
//! Klient, który widzi `prev` różne od swojego ostatniego `seq`, zgubił ramkę i prosi o brakujące
//! poleceniem `events_resume`. Ostatnie ramki trzyma [`EventLog`] (bufor o stałym budżecie); gdy
//! brakujących już w nim nie ma, odpowiedź ma `complete = false` i klient wczytuje stan od nowa.
//! `epoch` zmienia się z każdym uruchomieniem rdzenia — numery z poprzedniego uruchomienia nic
//! nie znaczą.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use crate::dto::{AlfaEvent, EventFrame, EventsResume};

/// Wersja ramki zdarzeń.
pub const FRAME_VERSION: u8 = 1;
/// Najwięcej ramek w buforze wznowienia.
pub const LOG_MAX_FRAMES: usize = 1024;
/// Najwięcej zdarzeń w buforze wznowienia (łącznie we wszystkich ramkach).
pub const LOG_MAX_EVENTS: usize = 8192;

/// Paczka zdarzeń wysyłana do UI jednym `emit` (ramka z numerami; odwołuje się jak `Vec`).
pub type EventBatch = Arc<EventFrame>;

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

#[derive(Default)]
struct Frames {
    frames: VecDeque<EventBatch>,
    events: usize,
    /// `seq` ostatniej wydanej ramki (0 — jeszcze żadnej).
    last: u64,
}

/// Bufor ostatnich ramek i licznik numerów (wspólny dla nadawców i zadania grupującego).
pub struct EventLog {
    epoch: String,
    /// Numer ostatnio wysłanego zdarzenia; zamek obejmuje też wstawienie do kolejki, żeby
    /// kolejność numerów zgadzała się z kolejnością w kolejce.
    issued: Mutex<u64>,
    frames: Mutex<Frames>,
}

impl EventLog {
    /// Nowy bufor z losowym znacznikiem uruchomienia.
    pub fn new() -> Self {
        Self {
            epoch: uuid::Uuid::new_v4().simple().to_string(),
            issued: Mutex::new(0),
            frames: Mutex::new(Frames::default()),
        }
    }

    /// Znacznik uruchomienia rdzenia.
    pub fn epoch(&self) -> &str {
        &self.epoch
    }

    /// Nadaje zdarzeniu numer i przekazuje je dalej pod zamkiem (kolejność = numeracja).
    pub fn issue(&self, send: impl FnOnce(u64)) {
        let mut issued = lock(&self.issued);
        *issued += 1;
        send(*issued);
    }

    /// Numer ostatnio wysłanego zdarzenia (może jeszcze czekać na swoją ramkę).
    pub fn issued(&self) -> u64 {
        *lock(&self.issued)
    }

    /// `seq` ostatniej wydanej ramki.
    pub fn latest(&self) -> u64 {
        lock(&self.frames).last
    }

    /// Zamyka paczkę w ramkę (`seq` = numer ostatniego zdarzenia, które do niej weszło)
    /// i zapisuje ją w buforze.
    pub fn seal(&self, seq: u64, events: Vec<AlfaEvent>) -> EventBatch {
        let mut log = lock(&self.frames);
        let frame = Arc::new(EventFrame {
            v: FRAME_VERSION,
            epoch: self.epoch.clone(),
            prev: log.last,
            seq,
            events,
        });
        log.last = seq;
        log.events += frame.events.len();
        log.frames.push_back(frame.clone());
        while log.frames.len() > LOG_MAX_FRAMES
            || (log.events > LOG_MAX_EVENTS && log.frames.len() > 1)
        {
            if let Some(old) = log.frames.pop_front() {
                log.events -= old.events.len();
            }
        }
        frame
    }

    /// Ramki po `after_seq`. `complete = false`, gdy klient jest z innego uruchomienia, podał
    /// numer, który nie jest granicą ramki, albo brakujące ramki wypadły już z bufora.
    pub fn resume(&self, after_seq: u64, epoch: Option<&str>) -> EventsResume {
        let log = lock(&self.frames);
        let lost = |frames: Vec<EventFrame>| EventsResume {
            epoch: self.epoch.clone(),
            latest_seq: log.last,
            complete: false,
            frames,
        };
        if epoch.is_some_and(|e| e != self.epoch) || after_seq > log.last {
            return lost(Vec::new());
        }
        let frames: Vec<EventFrame> = log
            .frames
            .iter()
            .filter(|frame| frame.seq > after_seq)
            .map(|frame| EventFrame::clone(frame))
            .collect();
        let complete = match frames.first() {
            Some(first) => first.prev == after_seq,
            None => after_seq == log.last,
        };
        if !complete {
            return lost(Vec::new());
        }
        EventsResume {
            epoch: self.epoch.clone(),
            latest_seq: log.last,
            complete: true,
            frames,
        }
    }
}

impl Default for EventLog {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
