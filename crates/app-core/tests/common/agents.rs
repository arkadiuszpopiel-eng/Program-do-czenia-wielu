//! Agentka z narzędziami w testach: rdzeń z atrapą wykonania poleceń (`platform-fake`), sesja
//! z katalogiem roboczym, Delta jako adresatka, bez samoweryfikacji, zgoda wygasa po 1 s.

use std::sync::Arc;
use std::time::Duration;

use app_core::dto::{
    AgentRunDetail, AlfaEvent, SendOptions, SessionTemplate, SettingValue, WorkdirChoice,
};
use app_core::ports::HeadlessShell;
use app_core::{AppCore, AppPaths};
use platform_fake::FakeExec;
use providers_contract::{ChatRequest, ContentBlock};

use super::{Harness, ScriptedProvider, ends, options, until};

/// Rdzeń z agentką Delta (Wykonawczyni) w sesji z katalogiem roboczym.
pub struct Agents {
    pub h: Harness,
    pub exec: Arc<FakeExec>,
    pub sid: String,
    pub workdir: std::path::PathBuf,
}

/// Rdzeń z atrapą wykonania poleceń, sesją z katalogiem roboczym i bez samoweryfikacji.
pub async fn agents() -> Agents {
    let dir = tempfile::tempdir().unwrap();
    let provider = Arc::new(ScriptedProvider::new(Duration::from_millis(1)));
    let shell = Arc::new(HeadlessShell::default());
    let exec = Arc::new(FakeExec::new());
    let mut opts = options(Some(provider.clone()), shell.clone());
    opts.exec = Some(exec.clone());
    opts.approval_timeout = Some(Duration::from_secs(1));
    let core = AppCore::build(AppPaths::under(dir.path()), opts)
        .await
        .unwrap();
    let mut a = agents_on(core, provider, shell, dir).await;
    a.exec = exec;
    a
}

/// Sesja z katalogiem roboczym i bez samoweryfikacji na gotowym rdzeniu (własne opcje testu).
pub async fn agents_on(
    core: AppCore,
    provider: Arc<ScriptedProvider>,
    shell: Arc<HeadlessShell>,
    dir: tempfile::TempDir,
) -> Agents {
    let rx = core.subscribe_events();
    core.settings_set(
        "agents.verify_before_done".into(),
        SettingValue::Bool(false),
    )
    .await
    .unwrap();
    let sid = core
        .sessions_create(SessionTemplate::Empty)
        .await
        .unwrap()
        .id;
    let chosen = core
        .sessions_choose_workdir(sid.clone(), WorkdirChoice::Default)
        .await
        .unwrap();
    let workdir = std::path::PathBuf::from(chosen.path.unwrap());
    Agents {
        h: Harness {
            core,
            provider,
            shell,
            rx,
            dir,
        },
        exec: Arc::new(FakeExec::new()),
        sid,
        workdir,
    }
}

/// Wiadomość do Delty.
pub fn to_delta(text: &str) -> SendOptions {
    SendOptions {
        parent_id: None,
        text: text.into(),
        addressed_to: Some("delta".into()),
        profile: None,
        attachments: Vec::new(),
    }
}

/// Teksty wyników narzędzi w żądaniu (to, co agentka dostała po kroku).
pub fn tool_results(request: &ChatRequest) -> String {
    request
        .messages
        .iter()
        .flat_map(|m| m.content.iter())
        .filter_map(|b| match b {
            ContentBlock::ToolResult(r) => Some(serde_json::to_string(r).unwrap()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Wysyła wiadomość i czeka na koniec tury agentki.
pub async fn run_turn(a: &mut Agents, text: &str) -> (String, Vec<AlfaEvent>) {
    let sent =
        a.h.core
            .turns_send(a.sid.clone(), to_delta(text))
            .await
            .unwrap();
    let turn = sent.assistant_turn_id.unwrap();
    let events = until(&mut a.h.rx, ends(&turn)).await;
    (turn, events)
}

/// Ostatni przebieg w Replay sesji.
pub async fn last_run(a: &Agents) -> AgentRunDetail {
    let runs = a.h.core.agents_runs(a.sid.clone()).await.unwrap();
    runs.last().cloned().expect("przebieg agentki w Replay")
}
