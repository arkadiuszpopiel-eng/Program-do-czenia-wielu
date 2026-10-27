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
