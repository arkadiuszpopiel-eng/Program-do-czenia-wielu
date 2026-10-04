//! Eval narzędzi F3 (`evals/F3/tools/tasks.json`): na CI — format zestawu i zadania ze skryptem
//! atrapy modelu przez `agent-runtime` z tymi samymi narzędziami co aplikacja (Broker na silniku
//! `safety-broker-impl`, dziennik cofania, prawdziwy system plików); pełny pomiar na modelu
//! lokalnym (llama.cpp) — test `#[ignore]` uruchamiany przez użytkownika (README zestawu).

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use app_agents::eval::{
    EvalEnv, EvalOptions, EvalTask, TaskKind, TaskSet, run_task, summarize, to_markdown,
};
use app_agents::{AgentTools, ShellToolsConfig, ToolsDeps};
use compliance_contract::{DenyLists, PathEnv};
use platform_contract::{ExecPort, FsPort};
use providers_contract::ModelProvider;
use providers_fake::{FAKE_MODEL, FakeProvider, Script};
use safety_broker_contract::{Broker, KernelPolicy};
use safety_broker_fake::FakeBroker;
use undo_journal_contract::UndoLimits;
use undo_journal_fake::FakeUndoJournal;
use watchdog_contract::ManualClock;

fn set_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../evals/F3/tools/tasks.json")
}

/// Środowisko jak w aplikacji: Broker z polityką bazową dla profilu `root`, dziennik cofania
/// nad prawdziwym systemem plików.
fn env(root: &Path, exec: Arc<dyn ExecPort>) -> EvalEnv {
    let profile = root.to_string_lossy().into_owned();
    let broker_dir = root.join("broker").to_string_lossy().into_owned();
    let policy = KernelPolicy::baseline(&profile, &broker_dir).unwrap();
    let paths = PathEnv::windows_profile(&profile);
    let broker = FakeBroker::with(policy, paths.clone(), Arc::new(ManualClock::new(1_000_000)));
    let fs: Arc<dyn FsPort> = Arc::new(platform_windows_impl::WindowsPlatform::default());
    let clock = Arc::new(|| 1_000_000u64);
    let journal = FakeUndoJournal::new(fs.clone(), UndoLimits::default(), clock).unwrap();
    EvalEnv {
        broker: Arc::new(broker.unwrap()) as Arc<dyn Broker>,
        journal: Arc::new(journal),
        fs,
        exec,
        env: paths,
        jobs: None,
    }
}

fn tool_names(env: &EvalEnv) -> Vec<String> {
    AgentTools::new(ToolsDeps {
        broker: env.broker.clone(),
        journal: env.journal.clone(),
        fs: env.fs.clone(),
        exec: env.exec.clone(),
        clipboard: None,
        env: env.env.clone(),
        deny: DenyLists::baseline(),
        jobs: None,
        bus: None,
        shell: ShellToolsConfig::default(),
        base_env: None,
        extra: Vec::new(),
        apps: None,
    })
    .names()
}

fn load(env: &EvalEnv) -> TaskSet {
    let json = std::fs::read_to_string(set_path()).unwrap();
    TaskSet::parse(&json, &tool_names(env)).unwrap()
}

/// Atrapa modelu odtwarzająca `ci_script` zadania, potem odpowiedź końcową.
fn scripted(task: &EvalTask) -> Arc<dyn ModelProvider> {
    let provider = FakeProvider::new("fake");
    for (i, call) in task.ci_script.iter().enumerate() {
        provider.push_script(Script::tool_call(
            FAKE_MODEL,
            &format!("c{i}"),
            &call.tool,
            &call.args,
        ));
    }
    let answer = task.ci_answer.clone().unwrap_or_else(|| "Gotowe.".into());
    provider.push_script(Script::text(FAKE_MODEL, &[answer.as_str()]));
    Arc::new(provider)
}

#[test]
fn task_set_format_is_valid() {
    let dir = tempfile::tempdir().unwrap();
    let env = env(dir.path(), Arc::new(platform_fake::FakeExec::new()));
    let set = load(&env);
    assert!(set.tasks.len() >= 30, "{} zadań", set.tasks.len());
    for kind in [TaskKind::Fs, TaskKind::Shell] {
        assert!(set.tasks.iter().any(|t| t.kind == kind), "{kind:?}");
    }
    let scripted = set.scripted();
    assert!(scripted.len() >= 3, "zadania CI: {}", scripted.len());
    for task in scripted {
        let args = serde_json::to_string(&task.ci_script).unwrap();
        assert!(
            args.is_ascii(),
            "{}: argumenty `ci_script` tylko ASCII",
            task.id
        );
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn scripted_tasks_pass_through_agent_runtime() {
    let dir = tempfile::tempdir().unwrap();
    let env = env(dir.path(), Arc::new(platform_fake::FakeExec::new()));
    let set = load(&env);
    let opts = EvalOptions {
        model: FAKE_MODEL.into(),
        verify: false,
        timeout: Duration::from_secs(30),
        max_steps: 12,
    };
    let root = dir.path().join("zadania");
    let mut results = Vec::new();
    for task in set.scripted() {
        results.push(run_task(task, &root, scripted(task), &env, &opts).await);
    }
    let report = to_markdown("atrapa", &results);
    eprintln!("{report}");
    let totals = summarize(&results);
    assert_eq!(totals.passed, totals.total, "{report}");
    assert!(results.iter().all(|r| r.tool_calls >= 1), "{results:#?}");
    // Zadanie z oczekiwanym stanem, którego skrypt nie realizuje — niezaliczone (runner nie
    // zalicza „na słowo” odpowiedzi).
    let mut wrong = set.scripted()[0].clone();
    wrong.id = "zly-skrypt".into();
    wrong.ci_script.clear();
    let failed = run_task(&wrong, &root, scripted(&wrong), &env, &opts).await;
    assert!(!failed.passed, "{failed:#?}");
}

/// Pełny pomiar na modelu lokalnym (Windows, llama.cpp). Zmienne środowiskowe:
/// `ALFA_EVAL_LLAMA_SERVER` (ścieżka `llama-server`), `ALFA_EVAL_MODELS_DIR` (katalog modeli
/// Alfy), `ALFA_EVAL_MODEL` (identyfikator z manifestu; domyślnie pierwszy), opcjonalnie
/// `ALFA_EVAL_ONLY` (`fs`/`shell`), `ALFA_EVAL_REPORT` (plik raportu Markdown).
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "pomiar na modelu lokalnym — uruchamia użytkownik (evals/F3/tools/README.md)"]
async fn measure_on_local_model() {
    use providers_local_impl::{LocalConfig, LocalProvider, Sidecar, TokioLauncher};
    let var = |name: &str| std::env::var(name).ok().filter(|v| !v.is_empty());
    let server = var("ALFA_EVAL_LLAMA_SERVER").expect("ALFA_EVAL_LLAMA_SERVER");
    let models_dir = var("ALFA_EVAL_MODELS_DIR").expect("ALFA_EVAL_MODELS_DIR");
    let models = providers_local_impl::builtin_models().unwrap();
    let model = var("ALFA_EVAL_MODEL").unwrap_or_else(|| models[0].id.clone());
    let sidecar = Sidecar::new(
        LocalConfig::new(models_dir, server),
        Arc::new(TokioLauncher::default()),
        None,
        None,
    )
    .unwrap();
    let provider: Arc<dyn ModelProvider> = Arc::new(LocalProvider::new(models, sidecar));
    let dir = tempfile::tempdir().unwrap();
    let exec: Arc<dyn ExecPort> = Arc::new(platform_windows_impl::WindowsPlatform::default());
    let env = env(dir.path(), exec);
    let set = load(&env);
    let only = var("ALFA_EVAL_ONLY");
    let opts = EvalOptions {
        model: model.clone(),
        ..EvalOptions::default()
    };
    let root = dir.path().join("zadania");
    let mut results = Vec::new();
    for task in &set.tasks {
        let kind = serde_json::to_value(task.kind).unwrap();
        if only.as_deref().is_some_and(|o| kind != o) {
            continue;
        }
        let r = run_task(task, &root, provider.clone(), &env, &opts).await;
        eprintln!("{}: {} {:?}", r.id, r.passed, r.failures);
        results.push(r);
    }
    let report = to_markdown(&model, &results);
    eprintln!("{report}");
    if let Some(path) = var("ALFA_EVAL_REPORT") {
        std::fs::write(path, &report).unwrap();
    }
}
