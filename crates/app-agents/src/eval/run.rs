//! Przebieg zadania zestawu przez `agent-runtime` (persona Delta w roli Wykonawczyni, katalog
//! roboczy = katalog zadania) i raport (Markdown, odsetek zaliczonych per rodzaj).

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

use agent_runtime_contract::{RunEvent, RunOutcome, StepKind};
use compliance_contract::{DenyLists, PathEnv};
use core_bus_contract::SessionId;
use personas_contract::{PersonaId, builtin_personas, builtin_roles};
use platform_contract::{ExecPort, FsPort};
use providers_contract::ModelProvider;
use risk_classifier_contract::CommandOrigin;
use safety_broker_contract::Broker;
use serde::{Deserialize, Serialize};
use tools_shell_contract::ShellToolsConfig;
use undo_journal_contract::UndoJournal;
use watchdog_contract::JobRegistry;

use super::{EvalTask, TaskKind, check, prepare};
use crate::runner::RunHandle;
use crate::spec::{AgentSettings, SpecInput, run_spec};
use crate::toolset::{AgentTools, ToolsDeps};

/// Środowisko wykonania (to samo co w aplikacji: Broker, dziennik cofania, platforma).
#[derive(Clone)]
pub struct EvalEnv {
    /// Broker.
    pub broker: Arc<dyn Broker>,
    /// Dziennik cofania.
    pub journal: Arc<dyn UndoJournal>,
    /// System plików.
    pub fs: Arc<dyn FsPort>,
    /// Wykonanie poleceń.
    pub exec: Arc<dyn ExecPort>,
    /// Środowisko ścieżek (profil).
    pub env: PathEnv,
    /// Rejestr Job Objects.
    pub jobs: Option<Arc<dyn JobRegistry>>,
}

/// Opcje przebiegu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvalOptions {
    /// Model u dostawcy.
    pub model: String,
    /// Samoweryfikacja (jak w aplikacji).
    pub verify: bool,
    /// Limit czasu zadania.
    pub timeout: Duration,
    /// Limit kroków.
    pub max_steps: u32,
}

impl Default for EvalOptions {
    fn default() -> Self {
        Self {
            model: "local".into(),
            verify: true,
            timeout: Duration::from_secs(300),
            max_steps: 24,
        }
    }
}

/// Wynik zadania.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskResult {
    /// Zadanie.
    pub id: String,
    /// Rodzaj.
    pub kind: TaskKind,
    /// Zaliczone.
    pub passed: bool,
    /// Niespełnione warunki.
    pub failures: Vec<String>,
    /// Wynik przebiegu.
    pub outcome: String,
    /// Kroki.
    pub steps: u32,
    /// Wywołania narzędzi.
    pub tool_calls: u32,
    /// Czas (ms).
    pub elapsed_ms: u64,
}

fn tools(env: &EvalEnv) -> AgentTools {
    AgentTools::new(ToolsDeps {
        broker: env.broker.clone(),
        journal: env.journal.clone(),
        fs: env.fs.clone(),
        exec: env.exec.clone(),
        clipboard: None,
        env: env.env.clone(),
        deny: DenyLists::baseline(),
        jobs: env.jobs.clone(),
        bus: None,
        shell: ShellToolsConfig::default(),
        base_env: None,
        extra: Vec::new(),
    })
}

fn outcome_name(o: Option<&RunOutcome>) -> String {
    match o {
        Some(RunOutcome::Completed { verified, .. }) => format!("completed(verified={verified:?})"),
        Some(other) => serde_json::to_value(other)
            .ok()
            .and_then(|v| v["outcome"].as_str().map(str::to_owned))
            .unwrap_or_else(|| "?".into()),
        None => "timeout".into(),
    }
}

/// Przechodzi zadanie w katalogu `root/<id>`.
pub async fn run_task(
    task: &EvalTask,
    root: &Path,
    provider: Arc<dyn ModelProvider>,
    env: &EvalEnv,
    opts: &EvalOptions,
) -> TaskResult {
    let started = Instant::now();
    let dir = root.join(&task.id);
    let mut result = TaskResult {
        id: task.id.clone(),
        kind: task.kind,
        passed: false,
        failures: Vec::new(),
        outcome: String::new(),
        steps: 0,
        tool_calls: 0,
        elapsed_ms: 0,
    };
    if let Err(e) = prepare(task, &dir) {
        result.failures.push(format!("stan początkowy: {e}"));
        return result;
    }
    let toolset = tools(env);
    let persona = builtin_personas()
        .into_iter()
        .find(|p| p.id == PersonaId::delta());
    let roles: Vec<_> = builtin_roles()
        .into_iter()
        .filter(|r| r.id.as_str() == "operator")
        .collect();
    let Some(persona) = persona else {
        result.failures.push("brak persony Delta".into());
        return result;
    };
    let settings = AgentSettings {
        max_steps: opts.max_steps,
        max_minutes: u32::try_from(opts.timeout.as_secs().div_ceil(60)).unwrap_or(u32::MAX),
        verify: opts.verify,
        approval_timeout_s: 1,
        ..AgentSettings::default()
    };
    let spec = run_spec(
        SpecInput {
            session: SessionId::new(format!("eval-{}", task.id)),
            persona,
            roles,
            goal: task.goal.clone(),
            origin: CommandOrigin::UserText,
            model: opts.model.clone(),
            tools: toolset.names(),
            workdir: dir.to_string_lossy().into_owned(),
            history: Vec::new(),
        },
        &settings,
        40_000,
        false,
    );
    let (_handle, mut feed) = match RunHandle::start(provider, &toolset, None, spec).await {
        Ok(x) => x,
        Err(e) => {
            result.failures.push(format!("start przebiegu: {e}"));
            return result;
        }
    };
    let mut outcome = None;
    let wait = tokio::time::timeout(opts.timeout, async {
        while let Some(env) = feed.next().await {
            match env.event {
                RunEvent::StepStarted { kind, .. } => {
                    result.steps += 1;
                    result.tool_calls += u32::from(kind == StepKind::Tool);
                }
                RunEvent::Finished { outcome: o } => outcome = Some(o),
                _ => {}
            }
        }
    })
    .await;
    if wait.is_err() {
        result
            .failures
            .push("przekroczony limit czasu zadania".into());
    }
    let answer = match &outcome {
        Some(RunOutcome::Completed { summary, .. }) => summary.clone(),
        _ => String::new(),
    };
    result.outcome = outcome_name(outcome.as_ref());
    result.failures.extend(check(task, &dir, &answer));
    result.passed = result.failures.is_empty();
    result.elapsed_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
    result
}

/// Podsumowanie.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Totals {
    /// Wszystkie.
    pub total: usize,
    /// Zaliczone.
    pub passed: usize,
    /// Per rodzaj: (zaliczone, wszystkie).
    pub by_kind: BTreeMap<String, (usize, usize)>,
}

impl Totals {
    /// Odsetek zaliczonych (0–100).
    pub fn rate_pct(&self) -> f64 {
        if self.total == 0 {
            return 0.0;
        }
        self.passed as f64 * 100.0 / self.total as f64
    }
}

/// Liczy podsumowanie.
pub fn summarize(results: &[TaskResult]) -> Totals {
    let mut t = Totals {
        total: results.len(),
        ..Totals::default()
    };
    for r in results {
        let key = serde_json::to_value(r.kind)
            .ok()
            .and_then(|v| v.as_str().map(str::to_owned))
            .unwrap_or_default();
        let e = t.by_kind.entry(key).or_default();
        e.1 += 1;
        if r.passed {
            t.passed += 1;
            e.0 += 1;
        }
    }
    t
}

/// Raport Markdown.
pub fn to_markdown(model: &str, results: &[TaskResult]) -> String {
    let t = summarize(results);
    let mut out = format!(
        "# Zestaw narzędzi F3 — {model}\n\nZaliczone: **{}/{}** ({:.1} %)\n\n",
        t.passed,
        t.total,
        t.rate_pct()
    );
    for (kind, (ok, all)) in &t.by_kind {
        out.push_str(&format!("- `{kind}`: {ok}/{all}\n"));
    }
    out.push_str(
        "\n| Zadanie | Wynik | Kroki | Narzędzia | Czas | Uwagi |\n|---|---|---|---|---|---|\n",
    );
    for r in results {
        out.push_str(&format!(
            "| `{}` | {} | {} | {} | {} ms | {} |\n",
            r.id,
            if r.passed { "✔" } else { "✘" },
            r.steps,
            r.tool_calls,
            r.elapsed_ms,
            r.failures.join("; ").replace('|', "/")
        ));
    }
    out
}
