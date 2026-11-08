//! Nadzór Marszałka nad zadaniami (na zdarzeniach `scheduler.*`/`triggers.*`): długie blokady,
//! przekroczone budżety, podejrzenie pętli, powtarzane porażki, przerwania siłą, terminy,
//! niedostarczony steering → eskalacje do użytkownika (z limitem); raport dzienny.

mod report;
mod text;

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use chrono::NaiveDate;
use core_bus_contract::Event;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use triggers_contract::Tz;

pub use report::DailyReport;

use text::{budget_pl, reason_pl};

/// Konfiguracja nadzoru.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct WatchConfig {
    /// Po ilu ms blokady (zasoby, brak agentki, limit) eskalować.
    pub stalled_after_ms: u64,
    /// Od której próby eskalować powtarzane porażki.
    pub retry_alert_attempt: u32,
    /// Najwięcej eskalacji na godzinę (nadmiar — tylko w raporcie dziennym).
    pub max_escalations_per_hour: usize,
    /// Strefa raportu dziennego.
    pub tz: Tz,
}

impl Default for WatchConfig {
    fn default() -> Self {
        Self {
            stalled_after_ms: 10 * 60_000,
            retry_alert_attempt: 3,
            max_escalations_per_hour: 20,
            tz: Tz::warsaw(),
        }
    }
}

/// Rodzaj ustalenia.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "finding", rename_all = "snake_case")]
pub enum FindingKind {
    /// Zadanie długo zablokowane.
    Blocked {
        /// Powód (`resources`, `no_agent`…).
        reason: String,
    },
    /// Przekroczony budżet zadania.
    BudgetExceeded {
        /// Który.
        budget: String,
    },
    /// Brak budżetu tła.
    BudgetBlocked,
    /// Powtarzane identyczne kroki.
    LoopSuspected,
    /// Kolejna porażka (ponowienie).
    RepeatedFailures {
        /// Próba.
        attempt: u32,
    },
    /// Przerwane siłą.
    Aborted,
    /// Minął termin.
    Expired,
    /// Wiadomość sterująca po końcu zadania.
    SteerUnconsumed,
    /// Wyzwalacz pominięty z powodu bezpieczeństwa (łańcuch, pętla, limit globalny).
    TriggerSuppressed {
        /// Powód.
        reason: String,
    },
}

/// Waga.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    /// Informacja (relacjonuje Dyrygentka przy okazji).
    Info,
    /// Ostrzeżenie (powiadomienie).
    Warn,
}

/// Eskalacja do użytkownika.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Escalation {
    /// Chwila (ms).
    pub at_ms: u64,
    /// Zadanie albo wyzwalacz.
    pub subject: String,
    /// Rodzaj.
    pub kind: FindingKind,
    /// Waga.
    pub severity: Severity,
    /// Komunikat po polsku.
    pub message: String,
}

#[derive(Debug, Clone, Default)]
struct Track {
    title: String,
    blocked: Option<(String, u64)>,
    escalated: BTreeSet<String>,
    last_ms: u64,
}

/// Po ilu ms bez zdarzeń zapominać zadanie (pamięć nadzoru jest ograniczona).
const FORGET_AFTER_MS: u64 = 24 * 3_600_000;

/// Nadzór.
#[derive(Debug, Clone, Default)]
pub struct Watch {
    cfg: WatchConfig,
    tasks: BTreeMap<String, Track>,
    days: BTreeMap<NaiveDate, DailyReport>,
    recent: VecDeque<u64>,
}

const STALL_REASONS: [&str; 5] = [
    "resources",
    "reserved",
    "no_agent",
    "agent_busy",
    "concurrency",
];

fn s<'a>(p: &'a Value, k: &str) -> &'a str {
    p.get(k).and_then(Value::as_str).unwrap_or_default()
}

impl Watch {
    /// Nadzór z konfiguracją.
    pub fn new(cfg: WatchConfig) -> Self {
        Self {
            cfg,
            ..Self::default()
        }
    }

    fn day_of(&self, at_ms: u64) -> Option<NaiveDate> {
        self.cfg
            .tz
            .to_local(i64::try_from(at_ms).ok()?)
            .map(|t| t.date())
    }

    fn day(&mut self, at_ms: u64) -> Option<&mut DailyReport> {
        let day = self.day_of(at_ms)?;
        let entry = self.days.entry(day).or_default();
        entry.day = Some(day);
        Some(entry)
    }

    fn escalate(
        &mut self,
        at_ms: u64,
        subject: &str,
        kind: FindingKind,
        severity: Severity,
        message: String,
    ) -> Option<Escalation> {
        let key = serde_json::to_string(&kind).unwrap_or_default();
        let track = self.tasks.entry(subject.to_owned()).or_default();
        if !track.escalated.insert(key) {
            return None;
        }
        self.recent.retain(|t| t + 3_600_000 > at_ms);
        let limited = self.recent.len() >= self.cfg.max_escalations_per_hour;
        if let Some(d) = self.day(at_ms) {
            if limited {
                d.escalations_suppressed += 1;
            } else {
                d.escalations += 1;
            }
        }
        if limited {
            return None;
        }
        self.recent.push_back(at_ms);
        Some(Escalation {
            at_ms,
            subject: subject.to_owned(),
            kind,
            severity,
            message,
        })
    }

    fn title(&self, task: &str) -> String {
        self.tasks
            .get(task)
            .map(|t| t.title.clone())
            .filter(|t| !t.is_empty())
            .unwrap_or_else(|| task.to_owned())
    }

    /// Zdarzenie magistrali → ewentualna eskalacja.
    pub fn on_event(&mut self, event: &Event) -> Vec<Escalation> {
        let p = &event.payload;
        let at = p.get("at_ms").and_then(Value::as_u64).unwrap_or_default();
        let task = s(p, "task").to_owned();
        let title = self.title(&task);
        if !task.is_empty() {
            self.tasks.entry(task.clone()).or_default().last_ms = at;
        }
        let mut out = Vec::new();
        match event.kind.as_str() {
            "scheduler.task.submitted" => {
                self.tasks.entry(task).or_default().title = s(p, "title").to_owned();
                if let Some(d) = self.day(at) {
                    d.submitted += 1;
                }
            }
            "scheduler.task.blocked" => {
                let reason = p
                    .get("reason")
                    .and_then(|r| r.get("blocked"))
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_owned();
                if let Some(d) = self.day(at) {
                    *d.blockers.entry(reason.clone()).or_default() += 1;
                }
                let t = self.tasks.entry(task).or_default();
                if t.blocked.as_ref().is_none_or(|(r, _)| *r != reason) {
                    t.blocked = Some((reason, at));
                }
            }
            "scheduler.task.dispatched" => {
                self.tasks.entry(task).or_default().blocked = None;
            }
            "scheduler.task.loop_suspected" => out.extend(self.escalate(
                at,
                &task,
                FindingKind::LoopSuspected,
                Severity::Warn,
                format!("Zadanie „{title}” powtarza te same kroki — możliwa pętla."),
            )),
            "scheduler.task.retry_scheduled" => {
                let attempt = p.get("attempt").and_then(Value::as_u64).unwrap_or_default();
                let attempt = u32::try_from(attempt).unwrap_or(u32::MAX);
                if attempt >= self.cfg.retry_alert_attempt {
                    out.extend(self.escalate(
                        at,
                        &task,
                        FindingKind::RepeatedFailures { attempt },
                        Severity::Info,
                        format!(
                            "Zadanie „{title}” zawiodło {} razy — próbuję ponownie.",
                            attempt - 1
                        ),
                    ));
                }
            }
            "scheduler.task.aborted" => out.extend(self.escalate(
                at,
                &task,
                FindingKind::Aborted,
                Severity::Warn,
                format!("Zadanie „{title}” nie odpowiadało i zostało przerwane."),
            )),
            "scheduler.task.steer_unconsumed" => out.extend(self.escalate(
                at,
                &task,
                FindingKind::SteerUnconsumed,
                Severity::Warn,
                format!("Wiadomość do zadania „{title}” przyszła po jego zakończeniu."),
            )),
            "scheduler.task.finished" => out.extend(self.finished(at, &task, &title, p)),
            "triggers.suppressed" => {
                let reason = p
                    .get("outcome")
                    .and_then(|o| o.get("reason"))
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_owned();
                if matches!(
                    reason.as_str(),
                    "chain_too_deep" | "self_loop" | "global_rate_limited"
                ) {
                    let trigger = s(p, "trigger").to_owned();
                    let message =
                        format!("Wyzwalacz „{trigger}” pominięty: {}.", reason_pl(&reason));
                    out.extend(self.escalate(
                        at,
                        &format!("wyzwalacz:{trigger}"),
                        FindingKind::TriggerSuppressed { reason },
                        Severity::Info,
                        message,
                    ));
                }
            }
            _ => {}
        }
        out
    }

    fn finished(&mut self, at: u64, task: &str, title: &str, p: &Value) -> Vec<Escalation> {
        let result = s(p, "result").to_owned();
        if let Some(d) = self.day(at) {
            match result.as_str() {
                "succeeded" => d.succeeded += 1,
                "failed" => d.failed += 1,
                "cancelled" => d.cancelled += 1,
                "expired" => d.expired += 1,
                "budget_exceeded" | "budget_blocked" => d.budget += 1,
                "skipped" => d.skipped += 1,
                _ => {}
            }
        }
        if let Some(t) = self.tasks.get_mut(task) {
            t.blocked = None;
        }
        let (kind, severity, message) = match result.as_str() {
            "budget_exceeded" => {
                let b = s(p, "budget").to_owned();
                let msg = format!("Zadanie „{title}” przekroczyło budżet {}.", budget_pl(&b));
                (
                    FindingKind::BudgetExceeded { budget: b },
                    Severity::Warn,
                    msg,
                )
            }
            "budget_blocked" => (
                FindingKind::BudgetBlocked,
                Severity::Info,
                format!("Zadanie „{title}” nie ruszyło — wyczerpany budżet tła."),
            ),
            "expired" => (
                FindingKind::Expired,
                Severity::Info,
                format!("Zadanie „{title}” nie zdążyło przed terminem."),
            ),
            _ => return Vec::new(),
        };
        self.escalate(at, task, kind, severity, message)
            .into_iter()
            .collect()
    }

    /// Upływ czasu: długie blokady.
    pub fn check(&mut self, now_ms: u64) -> Vec<Escalation> {
        self.tasks.retain(|_, t| {
            t.blocked.is_some() || t.last_ms.saturating_add(FORGET_AFTER_MS) > now_ms
        });
        if let Some(today) = self.day_of(now_ms) {
            self.days.retain(|d, _| (today - *d).num_days() <= 31);
        }
        let stalled: Vec<(String, String, u64)> = self
            .tasks
            .iter()
            .filter_map(|(id, t)| {
                let (reason, since) = t.blocked.as_ref()?;
                (STALL_REASONS.contains(&reason.as_str())
                    && now_ms.saturating_sub(*since) >= self.cfg.stalled_after_ms)
                    .then(|| (id.clone(), reason.clone(), *since))
            })
            .collect();
        let mut out = Vec::new();
        for (task, reason, since) in stalled {
            let title = self.title(&task);
            let minutes = now_ms.saturating_sub(since) / 60_000;
            let message = format!(
                "Zadanie „{title}” czeka od {minutes} min: {}.",
                reason_pl(&reason)
            );
            out.extend(self.escalate(
                now_ms,
                &task,
                FindingKind::Blocked { reason },
                Severity::Warn,
                message,
            ));
        }
        out
    }
}
