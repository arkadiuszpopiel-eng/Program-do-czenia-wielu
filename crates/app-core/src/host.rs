//! `TaskHost` rdzenia dla wykonawczyni zadań: sesja „Zadania w tle" (tworzona leniwie),
//! katalog roboczy i prywatność sesji, ustawienia agentek, zapis Replay i Osi czasu przebiegów
//! zadań (jak w czacie). Rdzeń trzymany słabo — scheduler nie przedłuża życia aplikacji.

use std::sync::{Arc, Weak};

use app_agents::{AgentSettings, Projection, RunOptions, RunSpec};
use app_tasks::{FactInput, TaskHost};
use async_trait::async_trait;
use core_config_contract::{ConfigKey, ConfigLayer, ConfigStore, Origin, Scope};
use cost_meter_contract::CostMeter;
use sessions_contract::{PrivacyTag, SessionCatalog, SessionId};

use crate::core::{AppCore, Inner};
use crate::dto::{AgentRun, SessionTemplate};
use crate::ids;

/// Klucz konfiguracji (warstwa maszyny): sesja zadań w tle.
pub(crate) const BACKGROUND_SESSION: &str = "tasks.background_session";
const BACKGROUND_TITLE: &str = "Zadania w tle";

/// Rdzeń widziany przez wykonawczynię.
pub(crate) struct CoreHost {
    inner: Weak<Inner>,
    create: tokio::sync::Mutex<()>,
}

impl CoreHost {
    pub fn new(core: &AppCore) -> Self {
        Self {
            inner: Arc::downgrade(&core.inner),
            create: tokio::sync::Mutex::new(()),
        }
    }

    fn core(&self) -> Option<AppCore> {
        self.inner.upgrade().map(|inner| AppCore { inner })
    }
}

#[async_trait]
impl TaskHost for CoreHost {
    async fn background_session(&self) -> Result<SessionId, String> {
        let core = self.core().ok_or("aplikacja zamknięta")?;
        let _guard = self.create.lock().await;
        if let Some(id) = core.config_str(BACKGROUND_SESSION).await
            && let Ok(id) = ids::session(&id)
            && core.ensure_session(&id).is_ok()
        {
            return Ok(id);
        }
        let created = core
            .sessions_create(SessionTemplate::Empty)
            .await
            .map_err(|e| e.message)?;
        core.sessions_rename(created.id.clone(), BACKGROUND_TITLE.into())
            .await
            .map_err(|e| e.message)?;
        let key = ConfigKey::new(BACKGROUND_SESSION).map_err(|e| e.to_string())?;
        core.inner
            .config
            .set(
                &key,
                Some(created.id.clone().into()),
                &Scope::Global,
                &ConfigLayer::Machine(core.inner.machine.clone()),
                Origin::Module("scheduler".into()),
            )
            .await
            .map_err(|e| e.to_string())?;
        ids::session(&created.id).map_err(|e| e.message)
    }

    /// Katalog roboczy zadania; nigdy niewybrany — katalog sesji (jak w rozmowie: bez niego zadanie
    /// w tle nie ma narzędzi, np. sieci dla „Kontroli cenników”). Jawne „bez katalogu” — `None`.
    fn workdir(&self, session: &SessionId) -> Option<String> {
        self.core()?.chat().session_workdir(session)
    }

    fn privacy(&self, session: &SessionId) -> PrivacyTag {
        self.core()
            .and_then(|c| c.inner.sessions.session(session).ok())
            .map_or(PrivacyTag::Private, |m| m.privacy)
    }

    async fn agent_settings(&self) -> AgentSettings {
        match self.core() {
            Some(core) => core.chat().agent_settings().await,
            None => AgentSettings::default(),
        }
    }

    fn usd_pln_e4(&self) -> u64 {
        self.core()
            .map_or(0, |c| u64::from(c.inner.costs.current_rate().rate_e4))
    }

    async fn prepare_agent_access(&self, spec: &mut RunSpec, options: &mut RunOptions) {
        if let Some(core) = self.core() {
            core.chat().prepare_agent_access(spec, options).await;
        } else {
            spec.tools.clear();
            options.crew = None;
        }
    }

    fn broker_window(&self) -> bool {
        self.core()
            .is_some_and(|c| c.inner.broker.approval_window())
    }

    async fn sync_taint(&self, session: &SessionId) -> Result<(), String> {
        match self.core() {
            Some(core) => core.chat().sync_taint(session).await.map_err(|e| e.message),
            None => Err("aplikacja zamknięta".into()),
        }
    }

    async fn facts(&self, session: &SessionId, own_task: &str) -> Option<String> {
        let core = self.core()?;
        let own_private = self.privacy(session) == PrivacyTag::Private;
        let tasks = app_tasks::visible_tasks(core.inner.tasks.tasks(), own_private, |s| {
            ids::session(s).map_or(true, |id| self.privacy(&id) == PrivacyTag::Private)
        });
        let workdir = app_tasks::read_workdir_facts(self.workdir(session)).await;
        Some(app_tasks::fact_sheet(&FactInput {
            now: chrono::Local::now().naive_local(),
            own_task: own_task.to_owned(),
            tasks,
            triggers: core.inner.tasks.triggers_list(),
            costs: Some(core.cost_summary(None).await),
            history: Some(core.cost_history()),
            workdir,
        }))
    }

    fn project(&self, session: &SessionId, run: &AgentRun, projection: Projection) {
        if let Some(core) = self.core() {
            core.chat().project_task(session, run, projection);
        }
    }
}
