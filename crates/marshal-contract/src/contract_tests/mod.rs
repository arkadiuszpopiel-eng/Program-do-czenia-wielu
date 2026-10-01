//! Współdzielone testy kontraktowe (feature `contract-tests`) — te same dla `-impl` i `-fake`.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs)]

mod rules;
mod watch;

use std::collections::BTreeMap;
use std::future::Future;
use std::sync::Mutex;

use async_trait::async_trait;
use core_bus_contract::{Event, Level};
use safety_broker_contract::{Capability, PathScope};
use serde_json::{Value, json};

use crate::{Ceiling, Marshal, RuleTranslator, event_kind};

pub use rules::{
    conflicts_are_reported, plan_examples_are_accepted, rules_only_narrow_and_need_user,
};
pub use watch::{daily_report, escalations_from_scheduler_events};

/// Uprząż testu.
#[async_trait]
pub trait Harness: Send + Sync {
    /// Typ Marszałka.
    type M: Marshal;
    /// Marszałek.
    fn marshal(&self) -> &Self::M;
    /// Nagrana odpowiedź tłumacza dla polecenia.
    fn script(&self, text: &str, drafts: Vec<Value>);
    /// Przesuwa czas o `ms`.
    async fn advance(&self, ms: u64);
    /// Bieżący czas (ms UTC).
    fn now_ms(&self) -> u64;
    /// Opublikowane zdarzenia.
    fn events(&self) -> Vec<Event>;
}

/// Tłumacz z nagranymi odpowiedziami (record/replay); polecenie bez nagrania = błąd.
#[derive(Default)]
pub struct ScriptedTranslator(Mutex<BTreeMap<String, Vec<Value>>>);

impl ScriptedTranslator {
    /// Nagrywa odpowiedź dla polecenia.
    pub fn script(&self, text: &str, drafts: Vec<Value>) {
        self.0
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .insert(text.to_owned(), drafts);
    }
}

#[async_trait]
impl RuleTranslator for ScriptedTranslator {
    async fn translate(&self, text: &str, _ceiling: &Ceiling) -> Result<Vec<Value>, String> {
        self.0
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .get(text)
            .cloned()
            .ok_or_else(|| format!("brak nagrania dla polecenia „{text}”"))
    }
}

/// Zakres poddrzewa ścieżki (kanoniczny).
pub fn tree(path: &str) -> PathScope {
    serde_json::from_value(json!({ "path": path, "subtree": true })).unwrap()
}

/// Sufit testów: L3, 40 kroków, 15 min, zapis w Pobranych, odczyt profilu, egress do example.com.
pub fn ceiling() -> Ceiling {
    Ceiling {
        capabilities: vec![
            Capability::FsWrite(tree("c:\\users\\ja\\downloads")),
            Capability::FsRead(tree("c:\\users\\ja")),
            Capability::NetEgress(serde_json::from_value(json!("*.example.com")).unwrap()),
        ],
        ..Ceiling::default()
    }
}

/// Zdarzenie schedulera/wyzwalaczy (syntetyczne).
pub fn ev(name: &str, payload: Value) -> Event {
    Event::new(event_kind(name), Level::Info, payload)
}

/// Cały zestaw; `factory(start_ms)` daje świeżą uprząż z sufitem [`ceiling`].
pub async fn run_all<H, F, Fut>(factory: F)
where
    H: Harness,
    F: Fn(u64) -> Fut,
    Fut: Future<Output = H>,
{
    let start = 1_790_841_600_000; // 2026-10-01 08:00 UTC
    rules_only_narrow_and_need_user(&factory(start).await).await;
    conflicts_are_reported(&factory(start).await).await;
    plan_examples_are_accepted(&factory(start).await).await;
    escalations_from_scheduler_events(&factory(start).await).await;
    daily_report(&factory(start).await).await;
}
