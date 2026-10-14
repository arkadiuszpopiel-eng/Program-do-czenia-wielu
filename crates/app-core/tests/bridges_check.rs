//! `bridges_check` (Ustawienia → Stan połączeń, Mosty CLI): stan logowania narzędzia z jego
//! oficjalnego polecenia stanu — zostaje tylko „zalogowano / nie”, sposób i plan; e-mail
//! i identyfikator organizacji z odpowiedzi `claude auth status` nie trafiają na kartę.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use accounts_hub_contract::{CliOutput, CliProbe};
use app_core::dto::BridgeAccountState;
use app_core::{AppCore, AppPaths};
use common::{ScriptedProvider, options};

/// Atrapa narzędzi: Claude Code zalogowany (plan Max), Codex niezalogowany, `agy` bez polecenia
/// stanu. Zapisuje wywołane polecenia.
#[derive(Default)]
struct Probe {
    calls: Mutex<Vec<String>>,
}

impl CliProbe for Probe {
    fn locate(&self, program: &str) -> Option<PathBuf> {
        ["claude", "codex", "agy"]
            .contains(&program)
            .then(|| PathBuf::from(format!(r"C:\narzedzia\{program}.exe")))
    }

    fn version_output(&self, _path: &Path) -> Option<String> {
        Some("1.0.0".into())
    }

    fn run_output(&self, path: &Path, args: &[&str]) -> Option<CliOutput> {
        let program = path.file_stem()?.to_string_lossy().into_owned();
        self.calls
            .lock()
            .unwrap()
            .push(format!("{program} {}", args.join(" ")));
        let (success, text) = match program.as_str() {
            "claude" => (
                true,
                r#"{"loggedIn": true, "authMethod": "claude.ai", "email": "ala@example.com",
                    "orgId": "org-577ee1b4", "subscriptionType": "max"}"#,
            ),
            "codex" => (false, "Not logged in"),
            _ => return None,
        };
        Some(CliOutput {
            success,
            text: text.into(),
        })
    }
}

async fn core_with(probe: Arc<Probe>) -> (AppCore, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let provider = Arc::new(ScriptedProvider::new(Duration::from_millis(1)));
    let shell = Arc::new(app_core::ports::HeadlessShell::default());
    let mut opts = options(Some(provider), shell);
    opts.cli_probe = Some(probe);
    let core = AppCore::build(AppPaths::under(dir.path()), opts)
        .await
        .unwrap();
    (core, dir)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn check_reports_login_plan_and_never_the_account_data() {
    let probe = Arc::new(Probe::default());
    let (core, _dir) = core_with(probe.clone()).await;

    let before = core.bridges_list(true).await.unwrap();
    assert!(
        before.iter().all(|c| c.account.is_none()),
        "bez sprawdzenia nie ma stanu konta"
    );

    let claude = core.bridges_check("claude_code".into()).await.unwrap();
    let account = claude.account.clone().expect("stan konta Claude Code");
    assert_eq!(account.state, BridgeAccountState::LoggedIn);
    assert_eq!(account.plan.as_deref(), Some("Max"));
    assert!(account.detail.contains("Alfa może zlecać zadania"));
    let json = serde_json::to_string(&claude).unwrap();
    assert!(
        !json.contains("ala@example.com") && !json.contains("org-577ee1b4"),
        "dane konta nie trafiają na kartę: {json}"
    );

    let codex = core.bridges_check("codex".into()).await.unwrap();
    assert_eq!(
        codex.account.map(|a| a.state),
        Some(BridgeAccountState::LoggedOut)
    );

    let agy = core.bridges_check("antigravity".into()).await.unwrap();
    assert_eq!(agy.route_id, "agy-antigravity");
    assert_eq!(
        agy.account.map(|a| a.state),
        Some(BridgeAccountState::Unknown)
    );

    // Wynik zostaje na karcie (lista bez ponownego sprawdzania).
    let after = core.bridges_list(false).await.unwrap();
    let kept = after
        .iter()
        .find(|c| c.bridge.as_deref() == Some("claude_code"))
        .and_then(|c| c.account.clone());
    assert_eq!(kept.map(|a| a.state), Some(BridgeAccountState::LoggedIn));

    // Tylko polecenia stanu — żadnego logowania ani zapytania do modelu; `agy` nie jest wywoływany.
    let calls = probe.calls.lock().unwrap().clone();
    assert_eq!(calls, ["claude auth status --json", "codex login status"]);

    assert!(core.bridges_check("nieznany".into()).await.is_err());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn check_without_the_tool_says_not_installed() {
    struct Empty;
    impl CliProbe for Empty {
        fn locate(&self, _program: &str) -> Option<PathBuf> {
            None
        }
        fn version_output(&self, _path: &Path) -> Option<String> {
            None
        }
    }
    let dir = tempfile::tempdir().unwrap();
    let provider = Arc::new(ScriptedProvider::new(Duration::from_millis(1)));
    let shell = Arc::new(app_core::ports::HeadlessShell::default());
    let mut opts = options(Some(provider), shell);
    opts.cli_probe = Some(Arc::new(Empty));
    let core = AppCore::build(AppPaths::under(dir.path()), opts)
        .await
        .unwrap();
    let card = core.bridges_check("claude_code".into()).await.unwrap();
    assert!(!card.detected);
    assert_eq!(
        card.account.map(|a| a.state),
        Some(BridgeAccountState::NotInstalled)
    );
}
