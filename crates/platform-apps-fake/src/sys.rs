//! Atrapa portu systemu: procesy z drzewem (PID → rodzic), usługi, zdarzenia i zmienne
//! w pamięci; te same polityki co Windows (strażnik celów, procesy krytyczne, tożsamość przy
//! zakończeniu, sekrety ukryte, zapis tylko zmiennych użytkownika). Dziennik wywołań
//! `terminate`/`control_service`/`set_user_env` do asercji („zero wywołań przy odmowie”).

use std::collections::BTreeMap;
use std::sync::{Mutex, MutexGuard};

use platform_apps_contract::{
    EnvScope, EnvVar, EventQuery, EventRecord, MAX_EVENT_MESSAGE_CHARS, MAX_EVENTS, ProcessDetails,
    ProcessEntry, ProcessIdentity, ServiceCommand, ServiceEntry, ServiceState, SysError, SysPort,
    check_env_name, check_env_value, check_provider, check_service_name, clip_chars, guard_env,
    is_protected_entry,
};
use platform_contract::TargetGuard;

/// Wywołanie zmieniające stan (dziennik atrapy).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SysCall {
    /// `terminate` doszło do „systemu” (proces zakończony).
    Terminated(u32),
    /// `control_service`.
    Service(String, ServiceCommand),
    /// `set_user_env` (nazwa, czy ustawiono wartość).
    SetEnv(String, bool),
}

#[derive(Debug, Default)]
struct State {
    processes: Vec<ProcessDetails>,
    services: Vec<ServiceEntry>,
    events: Vec<(EventQuery, EventRecord)>,
    env: BTreeMap<EnvScope, BTreeMap<String, String>>,
    calls: Vec<SysCall>,
    denied_services: Vec<String>,
    swap_on_terminate: Option<ProcessDetails>,
}

/// Atrapa `SysPort`.
#[derive(Debug)]
pub struct FakeSys {
    guard: TargetGuard,
    state: Mutex<State>,
}

impl Default for FakeSys {
    fn default() -> Self {
        Self::new(TargetGuard::baseline())
    }
}

/// Proces atrapy (`own = true`, sesja 1, czas startu = PID × 1000).
pub fn fake_process(pid: u32, parent_pid: u32, image: &str) -> ProcessDetails {
    ProcessDetails {
        entry: ProcessEntry {
            pid,
            parent_pid,
            image: image.to_owned(),
            session_id: Some(1),
            own: Some(true),
            threads: 4,
        },
        path: Some(format!(r"C:\Program Files\App\{image}")),
        started_ms: Some(u64::from(pid) * 1_000),
        memory_kb: Some(10_240),
        elevated: Some(false),
    }
}

impl FakeSys {
    /// Atrapa ze strażnikiem celów.
    pub fn new(guard: TargetGuard) -> Self {
        Self {
            guard,
            state: Mutex::new(State::default()),
        }
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|p| p.into_inner())
    }

    /// Dodaje proces.
    pub fn add_process(&self, p: ProcessDetails) {
        self.lock().processes.push(p);
    }

    /// Zastępuje proces o tym PID-zie innym (symulacja ponownego użycia PID-u).
    pub fn replace_process(&self, p: ProcessDetails) {
        let mut s = self.lock();
        s.processes.retain(|x| x.entry.pid != p.entry.pid);
        s.processes.push(p);
    }

    /// Podmienia proces tuż przed zakończeniem (wyścig: PID użyty ponownie po odczycie).
    pub fn replace_before_terminate(&self, p: ProcessDetails) {
        self.lock().swap_on_terminate = Some(p);
    }

    /// Dodaje usługę.
    pub fn add_service(&self, name: &str, state: ServiceState) {
        self.lock().services.push(ServiceEntry {
            name: name.to_owned(),
            display_name: format!("Usługa {name}"),
            state,
            pid: (state == ServiceState::Running).then_some(800),
        });
    }

    /// Usługa wymagająca administratora (sterowanie → `PermissionDenied`).
    pub fn deny_service(&self, name: &str) {
        self.lock().denied_services.push(name.to_ascii_lowercase());
    }

    /// Dodaje zdarzenie do dziennika `query.log`.
    pub fn add_event(&self, log: platform_apps_contract::EventLogName, record: EventRecord) {
        let q = EventQuery {
            log,
            min_level: None,
            provider: None,
            since_ms: None,
            max: 0,
        };
        self.lock().events.push((q, record));
    }

    /// Ustawia zmienną w zakresie.
    pub fn set_var(&self, scope: EnvScope, name: &str, value: &str) {
        self.lock()
            .env
            .entry(scope)
            .or_default()
            .insert(name.to_owned(), value.to_owned());
    }

    /// Dziennik wywołań zmieniających stan.
    pub fn calls(&self) -> Vec<SysCall> {
        self.lock().calls.clone()
    }

    fn entries(s: &State) -> Vec<ProcessEntry> {
        s.processes.iter().map(|p| p.entry.clone()).collect()
    }
}

impl SysPort for FakeSys {
    fn guard(&self) -> &TargetGuard {
        &self.guard
    }

    fn processes(&self) -> Result<Vec<ProcessEntry>, SysError> {
        Ok(Self::entries(&self.lock()))
    }

    fn process(&self, pid: u32) -> Result<ProcessDetails, SysError> {
        self.lock()
            .processes
            .iter()
            .find(|p| p.entry.pid == pid)
            .cloned()
            .ok_or_else(|| SysError::NotFound(format!("proces {pid}")))
    }

    fn terminate(&self, id: &ProcessIdentity) -> Result<(), SysError> {
        let mut s = self.lock();
        if let Some(p) = s.swap_on_terminate.take() {
            s.processes.retain(|x| x.entry.pid != p.entry.pid);
            s.processes.push(p);
        }
        let all = Self::entries(&s);
        let p = s
            .processes
            .iter()
            .find(|p| p.entry.pid == id.pid)
            .cloned()
            .ok_or_else(|| SysError::NotFound(format!("proces {}", id.pid)))?;
        if !id.matches(&p.entry.image, p.started_ms) {
            return Err(SysError::Changed(format!("PID {}", id.pid)));
        }
        let path_protected = p
            .path
            .as_deref()
            .is_some_and(|path| self.guard.is_protected(p.entry.pid, path));
        if path_protected || is_protected_entry(&self.guard, &p.entry, &all) {
            return Err(SysError::Protected(p.entry.image.clone()));
        }
        if p.entry.own != Some(true) {
            return Err(SysError::Protected(format!(
                "{} należy do innego użytkownika",
                p.entry.image
            )));
        }
        s.processes.retain(|x| x.entry.pid != id.pid);
        s.calls.push(SysCall::Terminated(id.pid));
        Ok(())
    }

    fn services(&self) -> Result<Vec<ServiceEntry>, SysError> {
        Ok(self.lock().services.clone())
    }

    fn control_service(
        &self,
        name: &str,
        command: ServiceCommand,
        _timeout_ms: u64,
    ) -> Result<ServiceEntry, SysError> {
        check_service_name(name)?;
        let mut s = self.lock();
        if s.denied_services.contains(&name.to_ascii_lowercase()) {
            return Err(SysError::PermissionDenied(format!(
                "usługa {name} wymaga administratora"
            )));
        }
        let svc = s
            .services
            .iter_mut()
            .find(|x| x.name.eq_ignore_ascii_case(name))
            .ok_or_else(|| SysError::NotFound(format!("usługa {name}")))?;
        svc.state = match command {
            ServiceCommand::Start | ServiceCommand::Restart => ServiceState::Running,
            ServiceCommand::Stop => ServiceState::Stopped,
        };
        svc.pid = (svc.state == ServiceState::Running).then_some(800);
        let out = svc.clone();
        s.calls.push(SysCall::Service(out.name.clone(), command));
        Ok(out)
    }

    fn events(&self, query: &EventQuery) -> Result<Vec<EventRecord>, SysError> {
        if let Some(p) = &query.provider {
            check_provider(p)?;
        }
        let s = self.lock();
        let mut out: Vec<EventRecord> = s
            .events
            .iter()
            .filter(|(q, r)| {
                q.log == query.log
                    && query.min_level.is_none_or(|min| r.level <= min)
                    && query
                        .provider
                        .as_ref()
                        .is_none_or(|p| p.eq_ignore_ascii_case(&r.provider))
            })
            .map(|(_, r)| EventRecord {
                message: clip_chars(&r.message, MAX_EVENT_MESSAGE_CHARS),
                ..r.clone()
            })
            .collect();
        out.sort_by(|a, b| b.time_ms.cmp(&a.time_ms));
        out.truncate(query.max.min(MAX_EVENTS) as usize);
        Ok(out)
    }

    fn env(&self, scope: EnvScope) -> Result<Vec<EnvVar>, SysError> {
        let s = self.lock();
        let vars = s
            .env
            .get(&scope)
            .map(|m| m.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
            .unwrap_or_default();
        Ok(guard_env(vars))
    }

    fn user_env_value(&self, name: &str) -> Result<Option<String>, SysError> {
        check_env_name(name)?;
        Ok(self
            .lock()
            .env
            .get(&EnvScope::User)
            .and_then(|m| m.get(name).cloned()))
    }

    fn set_user_env(&self, name: &str, value: Option<&str>) -> Result<Option<String>, SysError> {
        check_env_name(name)?;
        if let Some(v) = value {
            check_env_value(v)?;
        }
        let mut s = self.lock();
        let vars = s.env.entry(EnvScope::User).or_default();
        let previous = match value {
            Some(v) => vars.insert(name.to_owned(), v.to_owned()),
            None => vars.remove(name),
        };
        s.calls
            .push(SysCall::SetEnv(name.to_owned(), value.is_some()));
        Ok(previous)
    }
}
