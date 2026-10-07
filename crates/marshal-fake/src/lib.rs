//! Atrapa Marszałka (SPEC „Fake”): ten sam rdzeń co `-impl` ([`MarshalCore`]) z tłumaczem
//! record/replay ([`ScriptedTranslator`] — nagrane szkice reguł dla poleceń), wirtualnym zegarem
//! i nagranymi zdarzeniami. Do testów UI, Dyrygentki, poleceń głosowych — tylko jako
//! dev-dependency.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

use core_bus_contract::Event;
use marshal_contract::contract_tests::ScriptedTranslator;
use marshal_contract::{MarshalCore, MarshalHost, RuleBook, Watch, WatchConfig};

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

/// Otoczenie atrapy.
#[derive(Default)]
pub struct FakeMarshalHost {
    clock: AtomicU64,
    events: Mutex<Vec<Event>>,
    book: Mutex<Option<RuleBook>>,
}

impl MarshalHost for FakeMarshalHost {
    fn now_ms(&self) -> u64 {
        self.clock.load(Ordering::SeqCst)
    }

    fn emit(&self, events: Vec<Event>) {
        lock(&self.events).extend(events);
    }

    fn persist(&self, book: &RuleBook) {
        *lock(&self.book) = Some(book.clone());
    }
}

/// Marszałek-atrapa.
pub struct FakeMarshal {
    host: Arc<FakeMarshalHost>,
    translator: Arc<ScriptedTranslator>,
    core: MarshalCore<FakeMarshalHost>,
}

impl FakeMarshal {
    /// Nowa atrapa z zegarem `start_ms` i domyślnym sufitem.
    pub fn new(start_ms: u64) -> Self {
        Self::with_book(start_ms, RuleBook::default())
    }

    /// Atrapa z zapisaną księgą (stan po restarcie: reguły, sufit, propozycje).
    pub fn with_book(start_ms: u64, book: RuleBook) -> Self {
        let host = Arc::new(FakeMarshalHost::default());
        host.clock.store(start_ms, Ordering::SeqCst);
        *lock(&host.book) = Some(book.clone());
        let translator = Arc::new(ScriptedTranslator::default());
        let core = MarshalCore::new(
            Arc::clone(&host),
            translator.clone(),
            book,
            Watch::new(WatchConfig::default()),
        );
        Self {
            host,
            translator,
            core,
        }
    }

    /// Rdzeń (implementuje `Marshal`).
    pub fn core(&self) -> &MarshalCore<FakeMarshalHost> {
        &self.core
    }

    /// Nagrywa odpowiedź tłumacza.
    pub fn script(&self, text: &str, drafts: Vec<serde_json::Value>) {
        self.translator.script(text, drafts);
    }

    /// Przesuwa wirtualny zegar.
    pub fn advance(&self, ms: u64) {
        self.host.clock.fetch_add(ms, Ordering::SeqCst);
    }

    /// Bieżący czas.
    pub fn now_ms(&self) -> u64 {
        self.host.now_ms()
    }

    /// Nagrane zdarzenia.
    pub fn events(&self) -> Vec<Event> {
        lock(&self.host.events).clone()
    }

    /// Ostatnio zapisana księga reguł.
    pub fn stored(&self) -> Option<RuleBook> {
        lock(&self.host.book).clone()
    }

    /// Restart: nowa atrapa z ostatnio zapisanej księgi i tym samym zegarem (nadzór i nagrania
    /// tłumacza nie są trwałe).
    pub fn restarted(&self) -> Self {
        Self::with_book(self.now_ms(), self.stored().unwrap_or_default())
    }
}
