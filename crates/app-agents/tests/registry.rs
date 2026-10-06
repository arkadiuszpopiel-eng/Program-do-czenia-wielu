//! Rejestr narzędzi w aplikacji (fala 4): `AgentTools::kill_switch` zatrzymuje także odtwarzanie
//! multimediów (`MediaTools::stop_all` zestawu z `ToolsDeps::media` — tych samych narzędzi,
//! które dostają agentki); zapis zmiennej (`system_env_set`) daje kartę „Cofnij”
//! (`UndoService::System`), którą aplikacja cofa przez `AgentTools::undo_env`.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use app_agents::{AgentTools, AppsDeps, ShellToolsConfig, SysNetDeps, ToolsDeps};
use compliance_contract::{DenyLists, PathEnv};
use platform_apps_contract::{BrowserKind, BrowserSpec, SysPort};
use platform_apps_fake::{FakeBrowser, FakeDownloads, FakeOffice, FakeSys};
use platform_contract::FsPort;
use platform_fake::{FakeExec, FakeFs};
use safety_broker_contract::{Broker, Holder, KernelPolicy};
use safety_broker_fake::{FakeBroker, ScriptedDecision};
use serde_json::json;
use tools_common_contract::{Tool, ToolCtx, Toolset, UndoService};
use tools_media_contract::MediaToolsConfig;
use tools_media_fake::{FakePlayer, FakeTranscoder};
use tools_media_impl::{MediaTools, MediaToolsDeps};
use undo_journal_contract::{UndoJournal, UndoLimits};
use undo_journal_fake::FakeUndoJournal;
use watchdog_contract::ManualClock;

fn abs(rel: &str) -> PathBuf {
    let root = PathBuf::from(if cfg!(windows) { r"C:\" } else { "/" });
    rel.split('/')
        .filter(|s| !s.is_empty())
        .fold(root, |acc, seg| acc.join(seg))
}

/// Narzędzia systemowe nad `FakeSys` (zmienne użytkownika).
fn apps(sys: &Arc<FakeSys>) -> AppsDeps {
    let local = abs("Users/ala/AppData/Local/Alfa");
    AppsDeps {
        office: Arc::new(FakeOffice::new()),
        browser: Arc::new(FakeBrowser::new()),
        browser_spec: BrowserSpec {
            kind: BrowserKind::Edge,
            executable: None,
            alfa_root: local.clone(),
            profile_dir: local.join("browser").join("profile"),
            quarantine_dir: local.join("browser").join("quarantine"),
            headless: true,
        },
        plugins_dir: None,
        sysnet: Some(SysNetDeps {
            sys: sys.clone(),
            power: None,
            desktop: None,
            hardware: None,
            http: Arc::new(tools_net_fake::FakeHttp::default()),
            search: None,
            downloads: Arc::new(FakeDownloads::default()),
            quarantine_root: None,
        }),
    }
}

fn tools(player: Option<&FakePlayer>, sys: Option<&Arc<FakeSys>>) -> AgentTools {
    let fs: Arc<dyn FsPort> = Arc::new(FakeFs::default());
    let home = abs("Users/ala").to_string_lossy().into_owned();
    let env = PathEnv::windows_profile(&home);
    let policy = KernelPolicy::baseline(&home, "/ProgramData/AlfaBroker").unwrap();
    let fake =
        FakeBroker::with(policy, env.clone(), Arc::new(ManualClock::new(1_000_000))).unwrap();
    fake.script("tools-system.env_set", ScriptedDecision::Allow);
    let broker: Arc<dyn Broker> = Arc::new(fake);
    let journal: Arc<dyn UndoJournal> = Arc::new(
        FakeUndoJournal::new(fs.clone(), UndoLimits::default(), Arc::new(|| 1_000_000u64)).unwrap(),
    );
    let media = player.map(|p| {
        MediaTools::new(MediaToolsDeps {
            fs: fs.clone(),
            files: Arc::new(lib_media::StdFiles),
            journal: journal.clone(),
            transcoder: Arc::new(FakeTranscoder::new()),
            player: Arc::new(p.clone()),
            broker: broker.clone(),
            env: env.clone(),
            deny: DenyLists::baseline(),
            config: MediaToolsConfig::default(),
            bus: None,
        })
    });
    let extra: Vec<Arc<dyn Tool>> = media.as_ref().map(Toolset::tools).unwrap_or_default();
    AgentTools::new(ToolsDeps {
        broker,
        journal,
        fs,
        exec: Arc::new(FakeExec::new()),
        clipboard: None,
        env,
        deny: DenyLists::baseline(),
        jobs: None,
        bus: None,
        shell: ShellToolsConfig::default(),
        base_env: None,
        extra,
        apps: sys.map(apps),
        media,
    })
}

#[test]
fn kill_switch_stops_media_playback() {
    let player = FakePlayer::default();
    let tools = tools(Some(&player), None);
    assert!(tools.names().iter().any(|n| n == "media_play"));
    assert_eq!(player.stops(), 0);
    let _ = tools.kill_switch();
    assert_eq!(player.stops(), 1, "kill-switch zatrzymuje odtwarzanie");
    let _ = tools.kill_switch();
    assert_eq!(player.stops(), 2);
}

#[test]
fn kill_switch_without_media_stops_nothing() {
    assert_eq!(tools(None, None).kill_switch(), 0);
}

#[tokio::test]
async fn env_set_gives_system_undo_card_undone_by_the_registry() {
    let local = abs("Users/ala/AppData/Local/Alfa");
    let sys = Arc::new(FakeSys::new(app_agents::sysnet_guard(&local)));
    let tools = tools(None, Some(&sys));
    let set = tools
        .all()
        .into_iter()
        .find(|t| t.manifest().name == "system_env_set")
        .expect("system_env_set w rejestrze");
    let mut ctx = ToolCtx::new(Holder::agent("s1", "delta"));
    ctx.approval_timeout = Duration::from_millis(300);
    let out = set
        .call(json!({"name": "EDITOR", "value": "code"}), &ctx)
        .await;
    assert!(out.is_ok(), "{}", out.text);
    let undo = out.undo.clone().expect("karta „Cofnij”");
    assert_eq!(undo.service, UndoService::System);
    assert_eq!(
        sys.user_env_value("EDITOR").unwrap().as_deref(),
        Some("code")
    );
    assert!(tools.undo_env(undo.id).unwrap().contains("EDITOR"));
    assert_eq!(
        sys.user_env_value("EDITOR").unwrap(),
        None,
        "zmienna usunięta"
    );
    assert!(tools.undo_env(undo.id).is_err(), "karta jednorazowa");
}
