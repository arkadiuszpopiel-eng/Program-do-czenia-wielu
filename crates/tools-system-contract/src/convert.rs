//! Walidacja argumentów (ta sama w `-impl` i `-fake`) i przykładowe argumenty.

use platform_apps_contract::{
    check_env_name, check_env_value, check_provider, check_service_name, env_write_denied,
    is_critical_process, is_critical_service,
};
use serde::de::DeserializeOwned;

use crate::types::{
    EnvArgs, EnvSetArgs, EventsArgs, ProcessInfoArgs, ProcessKillArgs, ProcessesArgs,
    ServiceActionArg, ServiceControlArgs, ServicesArgs, StatusArgs,
};

fn parse<T: DeserializeOwned>(args: &serde_json::Value) -> Result<T, String> {
    serde_json::from_value(args.clone()).map_err(|e| e.to_string())
}

fn range(name: &str, v: Option<u32>, lo: u32, hi: u32) -> Result<(), String> {
    match v {
        Some(x) if !(lo..=hi).contains(&x) => Err(format!("`{name}` poza zakresem {lo}–{hi}")),
        _ => Ok(()),
    }
}

fn short(name: &str, v: Option<&String>, max: usize) -> Result<(), String> {
    match v {
        Some(s) if s.chars().count() > max || s.chars().any(char::is_control) => {
            Err(format!("`{name}` za długie albo ze znakami sterującymi"))
        }
        _ => Ok(()),
    }
}

/// Sprawdza argumenty narzędzia (schemat + zakresy + polityki składni portu).
pub fn check_args(tool: &str, args: &serde_json::Value) -> Result<(), String> {
    match tool {
        "system_processes" => {
            let a: ProcessesArgs = parse(args)?;
            range("limit", a.limit, 1, 500)?;
            short("name_contains", a.name_contains.as_ref(), 128)
        }
        "system_process_info" => parse::<ProcessInfoArgs>(args).map(|_| ()),
        "system_process_kill" => {
            let a: ProcessKillArgs = parse(args)?;
            if a.name.trim().is_empty() || a.name.chars().count() > 260 {
                return Err("`name` pusta albo za długa".into());
            }
            Ok(())
        }
        "system_services" => {
            let a: ServicesArgs = parse(args)?;
            range("limit", a.limit, 1, 500)?;
            short("name_contains", a.name_contains.as_ref(), 128)
        }
        "system_service_control" => {
            let a: ServiceControlArgs = parse(args)?;
            check_service_name(&a.name).map_err(|e| e.to_string())
        }
        "system_events" => {
            let a: EventsArgs = parse(args)?;
            range("since_hours", a.since_hours, 1, 720)?;
            range("max", a.max, 1, 200)?;
            match &a.provider {
                Some(p) => check_provider(p).map_err(|e| e.to_string()),
                None => Ok(()),
            }
        }
        "system_env" => {
            let a: EnvArgs = parse(args)?;
            short("name_contains", a.name_contains.as_ref(), 128)
        }
        "system_env_set" => {
            let a: EnvSetArgs = parse(args)?;
            check_env_name(&a.name).map_err(|e| e.to_string())?;
            match &a.value {
                Some(v) => check_env_value(v).map_err(|e| e.to_string()),
                None => Ok(()),
            }
        }
        "system_status" => parse::<StatusArgs>(args).map(|_| ()),
        other => Err(format!("nieznane narzędzie {other}")),
    }
}

/// Odmowa z polityki narzędzia niezależna od stanu systemu (ta sama w `-impl` i `-fake`):
/// zapis zmiennej z deny-listy, zakończenie PID-u systemowego albo procesu krytycznego,
/// zatrzymanie usługi krytycznej. Argumenty muszą być już sprawdzone [`check_args`].
pub fn policy_refusal(tool: &str, args: &serde_json::Value) -> Option<String> {
    match tool {
        "system_env_set" => {
            let a: EnvSetArgs = parse(args).ok()?;
            env_write_denied(&a.name).map(|why| format!("zapis zmiennej {} ({why})", a.name))
        }
        "system_process_kill" => {
            let a: ProcessKillArgs = parse(args).ok()?;
            (a.pid <= 4 || is_critical_process(&a.name)).then(|| {
                format!(
                    "zakończenie procesu {} (PID {}) — proces krytyczny systemu",
                    a.name, a.pid
                )
            })
        }
        "system_service_control" => {
            let a: ServiceControlArgs = parse(args).ok()?;
            (a.action != ServiceActionArg::Start && is_critical_service(&a.name)).then(|| {
                format!(
                    "zatrzymanie usługi {} — usługa Alfy, zabezpieczeń albo systemu",
                    a.name
                )
            })
        }
        _ => None,
    }
}

/// Przykładowe poprawne argumenty.
pub fn sample_args(tool: &str) -> serde_json::Value {
    match tool {
        "system_processes" => serde_json::json!({"name_contains": "note", "limit": 10}),
        "system_process_info" => serde_json::json!({"pid": 4321}),
        "system_process_kill" => serde_json::json!({"pid": 4321, "name": "notepad.exe"}),
        "system_services" => serde_json::json!({"state": "running"}),
        "system_service_control" => serde_json::json!({"name": "Spooler", "action": "restart"}),
        "system_events" => serde_json::json!({"log": "system", "level": "error", "max": 5}),
        "system_env" => serde_json::json!({"scope": "user"}),
        "system_env_set" => serde_json::json!({"name": "MOJA_ZMIENNA", "value": "1"}),
        _ => serde_json::json!({}),
    }
}
