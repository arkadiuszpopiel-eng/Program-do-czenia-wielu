//! `TasksApp`: komendy panelu Zadania (`tasks_*`), Wyzwalaczy (`triggers_*`) i Reguł Marszałka
//! (`marshal_*` w [`crate::rules`]). Zadania użytkownika mają pochodzenie `User`; ponowienie zachowuje pochodzenie
//! (zadania z wyzwalacza nie da się „wyprać" w żądanie użytkownika — most nadal odmówi).

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use app_api::dto::{
    CronPreview, NewTaskInput, TaskInfo, TriggerDraft, TriggerInfo, TriggerRunInfo,
};
use app_api::{AppError, EventHub};
use core_bus_contract::SessionId;
use marshal_contract::{Marshal, Proposal};
use marshal_impl::MarshalModule;
use personas_contract::PersonaId;
use scheduler_contract::{
    Assignee, Dependency, Scheduler, Steer, TaskClass, TaskId, TaskOrigin, TaskSpec, TimeWindow,
};
use scheduler_impl::SchedulerModule;
use triggers_contract::{Actor, CronExpr, TriggerId, Triggers, Tz};
use triggers_impl::TriggersModule;

use crate::map;

/// Moduły i stan współdzielony.
pub struct TasksParts {
    /// Pełny scheduler (wspólna tablica blokad z głosem).
    pub scheduler: Arc<SchedulerModule>,
    /// Wyzwalacze (`None` — moduł niepodłączony).
    pub triggers: Option<Arc<TriggersModule>>,
    /// Marszałek (`None` — moduł niepodłączony).
    pub marshal: Option<Arc<MarshalModule>>,
    /// Tłumacz poleceń (model) podłączony.
    pub translator: bool,
    /// Obserwacja katalogów dostępna na platformie.
    pub watch: bool,
    /// Polityka Marszałka: mosty zabronione (egzekwuje wykonawczyni).
    pub bridges_denied: Arc<AtomicBool>,
    /// Obsada schedulera i limity równoległości.
    pub roster: Arc<crate::bridge::RosterCtl>,
    /// Zdarzenia UI.
    pub events: EventHub,
}

/// Zadania, wyzwalacze, reguły.
pub struct TasksApp {
    pub(crate) p: TasksParts,
    seq: AtomicU64,
    pub(crate) proposals: Mutex<BTreeMap<u64, Proposal>>,
}

pub(crate) fn err(e: impl std::fmt::Display) -> AppError {
    AppError::invalid(e.to_string())
}

pub(crate) fn unavailable(what: &str, module: &str) -> AppError {
    AppError::unavailable(what, module)
}

fn now_ms() -> u64 {
    u64::try_from(chrono::Utc::now().timestamp_millis()).unwrap_or(0)
}

impl TasksApp {
    /// Nowa aplikacja zadań.
    pub fn new(parts: TasksParts) -> Self {
        Self {
            p: parts,
            seq: AtomicU64::new(0),
            proposals: Mutex::new(BTreeMap::new()),
        }
    }

    /// Scheduler (głos, kill-switch).
    pub fn scheduler(&self) -> Arc<SchedulerModule> {
        self.p.scheduler.clone()
    }

    /// Wyzwalacze.
    pub fn triggers(&self) -> Option<Arc<TriggersModule>> {
        self.p.triggers.clone()
    }

    pub(crate) fn proposals(&self) -> MutexGuard<'_, BTreeMap<u64, Proposal>> {
        self.proposals
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
    }

    pub(crate) fn next_id(&self, prefix: &str) -> String {
        let n = self.seq.fetch_add(1, Ordering::SeqCst);
        format!("{prefix}-{}-{n}", now_ms())
    }

    fn info(&self, id: &TaskId) -> Result<TaskInfo, AppError> {
        self.p
            .scheduler
            .task(id)
            .map(|v| map::task(&v))
            .ok_or_else(|| AppError::not_found(format!("Zadanie „{id}” nie istnieje.")))
    }

    /// `tasks_list`.
    pub fn tasks(&self) -> Vec<TaskInfo> {
        self.p.scheduler.tasks().iter().map(map::task).collect()
    }

    /// `tasks_create`: węzeł DAG od użytkownika (budżety zawężone polityką Marszałka).
    pub fn create(&self, input: NewTaskInput) -> Result<TaskInfo, AppError> {
        let goal = input.goal.trim();
        if goal.is_empty() || goal.chars().count() > 4_000 {
            return Err(AppError::invalid(
                "Cel zadania musi mieć od 1 do 4000 znaków.",
            ));
        }
        let title = Some(input.title.trim())
            .filter(|t| !t.is_empty())
            .map_or_else(|| goal.chars().take(60).collect(), str::to_owned);
        let assignee = match input.agent.as_deref().filter(|a| !a.is_empty()) {
            Some(a) => Assignee::Persona(PersonaId::new(a)),
            None => Assignee::AnyAgent,
        };
        let id = TaskId::new(self.next_id("u"));
        let mut spec = TaskSpec::new(
            id.clone(),
            title,
            assignee,
            TaskClass::User,
            TaskOrigin::User,
        );
        spec.deps = input
            .after
            .iter()
            .map(|t| Dependency::on(t.as_str()))
            .collect();
        spec.parent = input.parent_id.map(TaskId::new);
        spec.session = input.session_id.map(SessionId::new);
        spec.payload = serde_json::json!({ "goal": goal });
        if let Some(m) = &self.p.marshal {
            let policy = m.effective();
            spec.budget.max_steps = spec.budget.max_steps.min(policy.max_steps.max(1));
            spec.budget.max_wall_ms = spec.budget.max_wall_ms.min(policy.max_wall_ms.max(1));
        }
        self.p.scheduler.submit(vec![spec]).map_err(err)?;
        self.info(&id)
    }

    /// `tasks_cancel` (z poddrzewem delegacji).
    pub fn cancel(&self, task: &str) -> Result<Vec<String>, AppError> {
        let ids = self
            .p
            .scheduler
            .cancel(&TaskId::new(task), "anulowane przez użytkownika")
            .map_err(err)?;
        Ok(ids.iter().map(ToString::to_string).collect())
    }

    /// `tasks_retry`: nowe zadanie z tą samą specyfikacją (bez zależności), to samo pochodzenie.
    pub fn retry(&self, task: &str) -> Result<TaskInfo, AppError> {
        let view = self
            .p
            .scheduler
            .task(&TaskId::new(task))
            .ok_or_else(|| AppError::not_found(format!("Zadanie „{task}” nie istnieje.")))?;
        if !view.state.is_terminal() {
            return Err(AppError::invalid("Ponowić można tylko zakończone zadanie."));
        }
        let mut spec = view.spec;
        let base = spec
            .id
            .as_str()
            .split(".retry")
            .next()
            .unwrap_or(task)
            .to_owned();
        spec.id = TaskId::new(format!(
            "{base}.retry{}",
            self.seq.fetch_add(1, Ordering::SeqCst)
        ));
        spec.deps.clear();
        spec.parent = None;
        spec.window = TimeWindow {
            only_when_idle: spec.window.only_when_idle,
            not_in_game_mode: spec.window.not_in_game_mode,
            ..TimeWindow::default()
        };
        let id = spec.id.clone();
        self.p.scheduler.submit(vec![spec]).map_err(err)?;
        self.info(&id)
    }

    /// `tasks_steer`: wiadomość dla agentki w najbliższym punkcie atomowym.
    pub fn steer(&self, task: &str, text: &str) -> Result<(), AppError> {
        let text = text.trim();
        if text.is_empty() || text.chars().count() > 4_000 {
            return Err(AppError::invalid(
                "Wiadomość musi mieć od 1 do 4000 znaków.",
            ));
        }
        self.p
            .scheduler
            .steer(&TaskId::new(task), Steer::text(text))
            .map(|_| ())
            .map_err(err)
    }

    /// `tasks_pause`.
    pub fn pause(&self, task: &str) -> Result<(), AppError> {
        self.p.scheduler.pause(&TaskId::new(task)).map_err(err)
    }

    /// `tasks_resume`.
    pub fn resume(&self, task: &str) -> Result<(), AppError> {
        self.p.scheduler.resume(&TaskId::new(task)).map_err(err)
    }

    fn triggers_module(&self) -> Result<&Arc<TriggersModule>, AppError> {
        self.p
            .triggers
            .as_ref()
            .ok_or_else(|| unavailable("Wyzwalacze", "triggers"))
    }

    /// `triggers_list`.
    pub fn triggers_list(&self) -> Vec<TriggerInfo> {
        self.p.triggers.as_ref().map_or_else(Vec::new, |t| {
            t.list()
                .iter()
                .map(|v| map::trigger(v, self.p.watch))
                .collect()
        })
    }

    /// `triggers_create` (właściciel = użytkownik).
    pub fn trigger_create(&self, draft: &TriggerDraft) -> Result<TriggerInfo, AppError> {
        if draft.name.trim().is_empty() || draft.goal.trim().is_empty() {
            return Err(AppError::invalid("Podaj nazwę i cel wyzwalacza."));
        }
        let slug: String = draft
            .name
            .to_lowercase()
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
            .take(32)
            .collect();
        let id = format!(
            "{}-{}",
            slug.trim_matches('-'),
            self.seq.fetch_add(1, Ordering::SeqCst)
        );
        let spec = map::spec(id.trim_start_matches('-'), draft)?;
        let view = self
            .triggers_module()?
            .create(spec, Actor::User)
            .map_err(err)?;
        Ok(map::trigger(&view, self.p.watch))
    }

    /// `triggers_remove`.
    pub fn trigger_remove(&self, id: &str) -> Result<(), AppError> {
        self.triggers_module()?
            .remove(&TriggerId::new(id), Actor::User)
            .map_err(err)
    }

    /// `triggers_set_enabled`.
    pub fn trigger_set_enabled(&self, id: &str, enabled: bool) -> Result<(), AppError> {
        self.triggers_module()?
            .set_enabled(&TriggerId::new(id), enabled, Actor::User)
            .map_err(err)
    }

    /// `triggers_fire_now` („Uruchom teraz").
    pub fn trigger_fire(&self, id: &str) -> Result<TriggerRunInfo, AppError> {
        let record = self
            .triggers_module()?
            .fire_now(&TriggerId::new(id), Actor::User)
            .map_err(err)?;
        Ok(map::run(&record))
    }

    /// `triggers_log` (najnowsze na końcu).
    pub fn trigger_log(&self, id: Option<&str>) -> Vec<TriggerRunInfo> {
        let id = id.map(TriggerId::new);
        self.p.triggers.as_ref().map_or_else(Vec::new, |t| {
            t.log(id.as_ref(), 100).iter().map(map::run).collect()
        })
    }

    /// `triggers_preview_cron`: najbliższe uruchomienia (Europe/Warsaw, z DST).
    pub fn preview_cron(expr: &str, count: usize) -> CronPreview {
        match CronExpr::parse(expr) {
            Ok(cron) => {
                let tz = Tz::warsaw();
                let mut next = Vec::new();
                let mut at = now_ms();
                while next.len() < count.clamp(1, 10) {
                    match cron.next_after(at, &tz) {
                        Some(t) => {
                            next.push(map::iso_ms(t));
                            at = t;
                        }
                        None => break,
                    }
                }
                CronPreview {
                    valid: true,
                    error: None,
                    next,
                }
            }
            Err(e) => CronPreview {
                valid: false,
                error: Some(e.to_string()),
                next: Vec::new(),
            },
        }
    }
}
