//! `TaskHost` rdzenia dla wykonawczyni zadań: sesja „Zadania w tle" (tworzona leniwie),
//! katalog roboczy i prywatność sesji, ustawienia agentek, zapis Replay i Osi czasu przebiegów
//! zadań (jak w czacie). Rdzeń trzymany słabo — scheduler nie przedłuża życia aplikacji.

use std::sync::{Arc, Weak};

use app_agents::{AgentSettings, Projection};
use app_tasks::TaskHost;
use async_trait::async_trait;
use core_config_contract::{ConfigKey, ConfigLayer, ConfigStore, Origin, Scope};
use cost_meter_contract::CostMeter;
use sessions_contract::{PrivacyTag, SessionCatalog, SessionId};

use crate::core::{AppCore, Inner};
use crate::dto::{AgentRun, AlfaEvent, SessionTemplate};
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

impl AppCore {
    /// Projekcja przebiegu zadania: kroki i nagłówek do Replay, zdarzenia UI, Oś czasu.
    pub(crate) fn project_task(&self, session: &SessionId, run: &AgentRun, p: Projection) {
        let store = &self.inner.store;
        for step in &p.steps {
            if let Err(e) = store.push_step(session, &run.id, step) {
                tracing::warn!(error = %e, "zapis kroku zadania nie powiódł się");
            }
        }
        if p.run_changed
            && let Err(e) = store.push_run(session, run)
        {
            tracing::warn!(error = %e, "zapis przebiegu zadania nie powiódł się");
        }
        self.inner.events.emit_all(p.events);
        for event in p.timeline {
            if let Err(e) = store.push_timeline(session, &event) {
                tracing::warn!(error = %e, "zapis osi czasu nie powiódł się");
            }
            self.emit(AlfaEvent::TimelineAppended { event });
        }
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

    fn workdir(&self, session: &SessionId) -> Option<String> {
        self.core()?.inner.store.workdir(session).ok().flatten()
    }

    fn privacy(&self, session: &SessionId) -> PrivacyTag {
        self.core()
            .and_then(|c| c.inner.sessions.session(session).ok())
            .map_or(PrivacyTag::Private, |m| m.privacy)
    }

    async fn agent_settings(&self) -> AgentSettings {
        match self.core() {
            Some(core) => core.agent_settings().await,
            None => AgentSettings::default(),
        }
    }

    fn usd_pln_e4(&self) -> u64 {
        self.core()
            .map_or(0, |c| u64::from(c.inner.costs.current_rate().rate_e4))
    }

    fn broker_window(&self) -> bool {
        self.core()
            .is_some_and(|c| c.inner.broker.approval_window())
    }

    fn project(&self, session: &SessionId, run: &AgentRun, projection: Projection) {
        if let Some(core) = self.core() {
            core.project_task(session, run, projection);
        }
    }
}
