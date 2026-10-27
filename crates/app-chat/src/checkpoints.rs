//! Checkpoint runtime w tej samej szyfrowanej bazie co rozmowa i Replay.
use agent_runtime_contract::{Checkpoint, CheckpointError, CheckpointStore, RunId};
use app_store::AppStore;
use sessions_contract::SessionId;
use std::sync::Arc;

struct SessionStore {
    store: Arc<AppStore>,
    session: SessionId,
}
fn error(e: impl std::fmt::Display) -> CheckpointError {
    CheckpointError(e.to_string())
}
impl CheckpointStore for SessionStore {
    fn save(&self, cp: &Checkpoint) -> Result<(), CheckpointError> {
        let value = serde_json::to_value(cp).map_err(error)?;
        let turn = value["options"]["owner_turn"].as_str();
        self.store
            .save_work(&self.session, cp.run.as_str(), "runtime", turn, &value)
            .map_err(error)
    }
    fn latest(&self, run: &RunId) -> Result<Option<Checkpoint>, CheckpointError> {
        self.store
            .work(&self.session, run.as_str())
            .map_err(error)?
            .map(|v| serde_json::from_value(v).map_err(error))
            .transpose()
    }
    fn runs(&self) -> Result<Vec<RunId>, CheckpointError> {
        Ok(self
            .store
            .work_keys(&self.session, "runtime")
            .map_err(error)?
            .into_iter()
            .map(RunId::new)
            .collect())
    }
}
pub(crate) fn bind(deps: &crate::engine::ChatDeps) {
    if let Some(agents) = &deps.agents {
        let store = deps.store.clone();
        agents.launch.bind_checkpoints(move |session| {
            Arc::new(SessionStore {
                store: store.clone(),
                session: session.clone(),
            })
        });
    }
}
