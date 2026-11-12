//! Syntetyczne, szyfrowane bazy — kontrola źródła i granic projekcji, bez wykonania narzędzi.
use crate::{AppStore, TurnMeta};
use app_api::{dto::*, ids};
use lib_sqlstore::{Db, DbKey};
use sessions_contract::{SessionDbProvider, SessionError, SessionId, TurnId};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

pub(super) const SECRET: &str = "NEVER_SEND_THIS_9f1";
pub(super) struct Provider(pub(super) BTreeMap<SessionId, Arc<Db>>);
impl SessionDbProvider for Provider {
    fn session_db(&self, id: &SessionId) -> Result<Arc<Db>, SessionError> {
        self.0
            .get(id)
            .cloned()
            .ok_or_else(|| SessionError::storage("controlled fixture error"))
    }
    fn session_ids(&self) -> Result<Vec<SessionId>, SessionError> {
        Ok(self.0.keys().cloned().collect())
    }
}
pub(super) struct Fixture {
    pub(super) store: AppStore,
    pub(super) sid: SessionId,
    _dir: tempfile::TempDir,
}
impl Fixture {
    pub(super) fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let dbs = ["s", "other"]
            .into_iter()
            .map(|id| {
                (
                    SessionId::new(id),
                    Arc::new(Db::open(&dir.path().join(id), &DbKey::from_bytes([7; 32])).unwrap()),
                )
            })
            .collect();
        Self {
            _dir: dir,
            store: AppStore::new(Arc::new(Provider(dbs))),
            sid: SessionId::new("s"),
        }
    }
    pub(super) fn meta(&self, turn: u64, tools: Vec<ToolStep>) {
        self.store
            .put_meta(
                &self.sid,
                TurnId(turn),
                &TurnMeta {
                    agent: Some("alfa".into()),
                    tools,
                    ..Default::default()
                },
            )
            .unwrap();
    }
    pub(super) fn records(&self, ids: &[u64]) -> crate::ExecutionRecords {
        self.store
            .execution_records(
                &self.sid,
                &ids.iter().copied().map(TurnId).collect::<Vec<_>>(),
                &BTreeSet::from(["fs_read".into(), "fs_write".into()]),
            )
            .unwrap()
            .unwrap()
    }
    pub(super) fn set_header(&self, raw: &str, agent: &str, bridge: Option<&str>) {
        let id = ids::run_dto(&self.sid, raw);
        let mut run = self
            .store
            .runs(&self.sid)
            .unwrap()
            .into_iter()
            .find(|detail| detail.run.id == id)
            .unwrap()
            .run;
        run.agent = agent.into();
        run.bridge = bridge.map(str::to_owned);
        self.store.push_run(&self.sid, &run).unwrap();
    }
    pub(super) fn set_step(&self, raw: &str, name: &str, kind: ReplayKind) {
        let id = ids::run_dto(&self.sid, raw);
        let mut step = self
            .store
            .runs(&self.sid)
            .unwrap()
            .into_iter()
            .find(|detail| detail.run.id == id)
            .unwrap()
            .steps
            .remove(0);
        step.tool = Some(name.into());
        step.kind = kind;
        self.store.push_step(&self.sid, &id, &step).unwrap();
    }
    pub(super) fn replay(
        &self,
        raw: &str,
        turn: u64,
        status: ReplayStatus,
        bridge: Option<&str>,
        checkpoint: bool,
    ) -> String {
        let run = AgentRun {
            id: ids::run_dto(&self.sid, raw),
            session_id: self.sid.to_string(),
            turn_id: Some(ids::turn_dto(&self.sid, TurnId(turn))),
            agent: "alfa".into(),
            goal: SECRET.into(),
            workdir: Some(SECRET.into()),
            state: RunState::Completed,
            started_at: "2026-11-11T00:00:00Z".into(),
            finished_at: None,
            summary: Some(SECRET.into()),
            usage: RunUsage {
                steps: 1,
                tool_calls: 1,
                input_tokens: 0,
                output_tokens: 0,
                cost: Money::pln(0),
                elapsed_ms: 1,
            },
            budget: RunBudgetView {
                max_steps: 10,
                max_minutes: 1,
                max_cost: None,
            },
            bridge: bridge.map(str::to_owned),
            task_id: None,
            parent_id: None,
            label: Some(SECRET.into()),
        };
        self.store.push_run(&self.sid, &run).unwrap();
        let id = ids::step_dto(&self.sid, raw, 1);
        self.store
            .push_step(
                &self.sid,
                &run.id,
                &ReplayStep {
                    id: id.clone(),
                    n: 1,
                    kind: ReplayKind::Tool,
                    tool: Some("fs_write".into()),
                    title: SECRET.into(),
                    input: SECRET.into(),
                    output: SECRET.into(),
                    status,
                    at_ms: 1,
                    duration_ms: Some(1),
                    undo_token: None,
                    undone: false,
                    untrusted: true,
                    intent: None,
                    approval_id: None,
                },
            )
            .unwrap();
        if checkpoint {
            self.store
                .save_work(
                    &self.sid,
                    raw,
                    "runtime",
                    run.turn_id.as_deref(),
                    &serde_json::json!({"do_not_read_messages":SECRET}),
                )
                .unwrap();
        }
        id
    }
}
pub(super) fn step(id: &str, status: ToolStatus) -> ToolStep {
    ToolStep {
        id: id.into(),
        icon: ToolIcon::Terminal,
        label: SECRET.into(),
        status,
        duration_ms: None,
        undo_token: None,
        undone: false,
        intent: None,
    }
}
