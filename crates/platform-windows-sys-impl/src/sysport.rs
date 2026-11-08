//! [`WinSys`] — `SysPort` dla Windows (`tools-system`): procesy (`procs_win`), usługi
//! (`services_win`), Dziennik zdarzeń (`events_win`), zmienne (`env_win`). Polityki portu
//! (strażnik celów, procesy i usługi krytyczne, deny-lista zapisu zmiennych, ukrywanie sekretów,
//! XPath z wartości sprawdzonych) stosowane tu niezależnie od narzędzia — obrona w głąb.
//! Poza Windows: procesy, usługi, zdarzenia i rejestr → `Unsupported`; środowisko procesu działa.

use platform_apps_contract::{
    EnvScope, EnvVar, EventQuery, EventRecord, MAX_EVENTS, MAX_SERVICE_WAIT_MS, ProcessDetails,
    ProcessEntry, ProcessIdentity, ServiceCommand, ServiceEntry, SysError, SysPort, check_env_name,
    check_env_value, check_service_name, env_write_denied, event_xpath, guard_env,
    is_critical_service,
};
use platform_contract::TargetGuard;

#[cfg(not(windows))]
mod imp {
    use super::*;

    fn unsupported<T>(what: &str) -> Result<T, SysError> {
        Err(SysError::Unsupported(format!("{what} tylko na Windows")))
    }

    pub(super) fn processes() -> Result<Vec<ProcessEntry>, SysError> {
        unsupported("lista procesów")
    }
    pub(super) fn process(_pid: u32) -> Result<ProcessDetails, SysError> {
        unsupported("szczegóły procesu")
    }
    pub(super) fn terminate(_g: &TargetGuard, _id: &ProcessIdentity) -> Result<(), SysError> {
        unsupported("zakończenie procesu")
    }
    pub(super) fn services() -> Result<Vec<ServiceEntry>, SysError> {
        unsupported("usługi")
    }
    pub(super) fn control(_n: &str, _c: ServiceCommand, _t: u64) -> Result<ServiceEntry, SysError> {
        unsupported("sterowanie usługą")
    }
    pub(super) fn query(_c: &str, _x: &str, _m: u32) -> Result<Vec<EventRecord>, SysError> {
        unsupported("Dziennik zdarzeń")
    }
    pub(super) fn read(_s: EnvScope) -> Result<Vec<(String, String)>, SysError> {
        unsupported("zmienne w rejestrze")
    }
    pub(super) fn user_value(_n: &str) -> Result<Option<String>, SysError> {
        unsupported("zmienne w rejestrze")
    }
    pub(super) fn set_user(_n: &str, _v: Option<&str>) -> Result<Option<String>, SysError> {
        unsupported("zapis zmiennych")
    }
}

#[cfg(windows)]
mod imp {
    pub(super) use crate::env_win::{read, set_user, user_value};
    pub(super) use crate::events_win::query;
    pub(super) use crate::procs_win::{process, processes, terminate};
    pub(super) use crate::services_win::{control, services};
}

/// Port systemu Windows ze strażnikiem celów (drzewo procesów Alfy, Broker, watchdog).
#[derive(Debug, Clone)]
pub struct WinSys {
    guard: TargetGuard,
}

impl WinSys {
    /// Port ze strażnikiem celów.
    pub fn new(guard: TargetGuard) -> Self {
        Self { guard }
    }
}

impl SysPort for WinSys {
    fn guard(&self) -> &TargetGuard {
        &self.guard
    }

    fn processes(&self) -> Result<Vec<ProcessEntry>, SysError> {
        imp::processes()
    }

    fn process(&self, pid: u32) -> Result<ProcessDetails, SysError> {
        imp::process(pid)
    }

    fn terminate(&self, id: &ProcessIdentity) -> Result<(), SysError> {
        imp::terminate(&self.guard, id)
    }

    fn services(&self) -> Result<Vec<ServiceEntry>, SysError> {
        imp::services()
    }

    fn control_service(
        &self,
        name: &str,
        command: ServiceCommand,
        timeout_ms: u64,
    ) -> Result<ServiceEntry, SysError> {
        check_service_name(name)?;
        if command != ServiceCommand::Start && is_critical_service(name) {
            return Err(SysError::Protected(format!(
                "usługa {name} (Alfa, zabezpieczenia, system)"
            )));
        }
        imp::control(name, command, timeout_ms.min(MAX_SERVICE_WAIT_MS))
    }

    fn events(&self, query: &EventQuery) -> Result<Vec<EventRecord>, SysError> {
        let xpath = event_xpath(query)?;
        imp::query(query.log.channel(), &xpath, query.max.clamp(1, MAX_EVENTS))
    }

    fn env(&self, scope: EnvScope) -> Result<Vec<EnvVar>, SysError> {
        let raw = match scope {
            EnvScope::Process => std::env::vars_os()
                .map(|(k, v)| {
                    (
                        k.to_string_lossy().into_owned(),
                        v.to_string_lossy().into_owned(),
                    )
                })
                .collect(),
            other => imp::read(other)?,
        };
        Ok(guard_env(raw))
    }

    fn user_env_value(&self, name: &str) -> Result<Option<String>, SysError> {
        check_env_name(name)?;
        imp::user_value(name)
    }

    fn set_user_env(&self, name: &str, value: Option<&str>) -> Result<Option<String>, SysError> {
        check_env_name(name)?;
        if let Some(why) = env_write_denied(name) {
            return Err(SysError::Protected(format!("zmienna {name}: {why}")));
        }
        if let Some(v) = value {
            check_env_value(v)?;
        }
        imp::set_user(name, value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use platform_apps_contract::EventLogName;

    #[test]
    fn port_policies_hold_before_any_system_call() {
        let s = WinSys::new(TargetGuard::baseline());
        for name in ["AlfaBroker", "WinDefend", "eventlog"] {
            assert!(matches!(
                s.control_service(name, ServiceCommand::Stop, 1),
                Err(SysError::Protected(_))
            ));
        }
        assert!(matches!(
            s.control_service("a\\b", ServiceCommand::Start, 1),
            Err(SysError::Invalid(_))
        ));
        for name in [
            "LOCALAPPDATA",
            "WEBVIEW2_X",
            "OPENAI_API_KEY",
            "HTTPS_PROXY",
        ] {
            assert!(matches!(
                s.set_user_env(name, Some("x")),
                Err(SysError::Protected(_))
            ));
        }
        let bad = EventQuery {
            log: EventLogName::System,
            min_level: None,
            provider: Some("x' or '1'='1".into()),
            since_ms: None,
            max: 5,
        };
        assert!(matches!(s.events(&bad), Err(SysError::Invalid(_))));
        let env = s.env(EnvScope::Process).unwrap();
        assert!(
            env.iter()
                .all(|v| !platform_apps_contract::is_secret_env_name(&v.name) || v.value.is_none())
        );
        assert_eq!(s.guard(), &TargetGuard::baseline());
    }

    #[cfg(not(windows))]
    #[test]
    fn system_queries_are_unsupported_off_windows() {
        let s = WinSys::new(TargetGuard::baseline());
        assert!(matches!(s.processes(), Err(SysError::Unsupported(_))));
        assert!(matches!(s.services(), Err(SysError::Unsupported(_))));
        assert!(matches!(
            s.env(EnvScope::User),
            Err(SysError::Unsupported(_))
        ));
    }
}
