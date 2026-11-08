//! Silnik gier NextGO.Gray w aplikacji (`tools-engine`): katalog silnika z ustawienia
//! `engine.nextgo.root` (odświeżany po zmianie w Ustawieniach), port nad prawdziwym serwerem,
//! zrzuty i podglądy w `<dane lokalne>/engine`.

use std::path::{Path, PathBuf};
use std::sync::{Arc, PoisonError, RwLock};

use compliance_contract::{DenyLists, PathEnv};
use core_config_contract::{ConfigKey, ConfigStore, Scope};
use futures_util::StreamExt;
use safety_broker_contract::Broker;
use tools_engine_contract::{EnginePort, ROOT_SETTING};
pub use tools_engine_impl::EngineRoot;
use tools_engine_impl::{EngineTools, EngineToolsDeps, ProcessEngine};

/// Katalog z wartości ustawienia: pusty — brak; `%ZMIENNA%` rozwinięte ze środowiska.
pub fn root_from_setting(value: &str, vars: impl Fn(&str) -> Option<String>) -> Option<PathBuf> {
    let mut out = String::new();
    let mut rest = value.trim().trim_matches('"');
    while let Some(start) = rest.find('%') {
        out.push_str(&rest[..start]);
        let tail = &rest[start + 1..];
        match tail.find('%') {
            Some(end) => {
                let name = &tail[..end];
                out.push_str(&vars(name).unwrap_or_else(|| format!("%{name}%")));
                rest = &tail[end + 1..];
            }
            None => {
                out.push('%');
                rest = tail;
            }
        }
    }
    out.push_str(rest);
    let path = PathBuf::from(out.trim());
    (!path.as_os_str().is_empty() && path.is_absolute()).then_some(path)
}

/// Katalog silnika śledzący ustawienie (zadanie w tle kończy się z magazynem ustawień).
pub fn watch_root(config: Arc<dyn ConfigStore>) -> EngineRoot {
    let root: EngineRoot = Arc::new(RwLock::new(None));
    let shared = root.clone();
    tokio::spawn(async move {
        let Ok(key) = ConfigKey::new(ROOT_SETTING) else {
            return;
        };
        let mut changes = config.watch(ROOT_SETTING);
        loop {
            let value = config.get(&key, &Scope::Global).await.ok().flatten();
            let path = value
                .as_ref()
                .and_then(|v| v.as_str())
                .and_then(|s| root_from_setting(s, |n| std::env::var(n).ok()));
            *shared.write().unwrap_or_else(PoisonError::into_inner) = path;
            if changes.next().await.is_none() {
                break;
            }
        }
    });
    root
}

/// Narzędzia silnika nad prawdziwym serwerem (`local` — katalog danych lokalnych Alfy) albo nad
/// portem podanym w `port` (atrapa w testach rdzenia).
pub fn engine_tools(
    (root, port): (EngineRoot, Option<Arc<dyn EnginePort>>),
    local: &Path,
    broker: Arc<dyn Broker>,
    env: PathEnv,
    deny: DenyLists,
) -> EngineTools {
    let port = port.unwrap_or_else(|| {
        Arc::new(ProcessEngine::new(
            root,
            local.join("engine"),
            std::env::vars().collect(),
        ))
    });
    EngineTools::new(EngineToolsDeps {
        port,
        broker,
        env,
        deny,
    })
}

#[cfg(test)]
mod tests {
    use super::root_from_setting;

    #[test]
    fn setting_value_becomes_an_absolute_path() {
        let vars = |n: &str| (n == "USERPROFILE").then(|| "C:\\Users\\A".to_owned());
        assert_eq!(root_from_setting("", vars), None);
        assert_eq!(root_from_setting("  ", vars), None);
        assert_eq!(
            root_from_setting("silnik", vars),
            None,
            "tylko ścieżka pełna"
        );
        let p = root_from_setting("\"%USERPROFILE%\\Gry\\NextGO\"", vars).unwrap();
        assert_eq!(p.to_string_lossy(), "C:\\Users\\A\\Gry\\NextGO");
        let raw = root_from_setting("E:\\Silnik Do Gier AI NextGO.Gray", vars).unwrap();
        assert!(raw.ends_with("Silnik Do Gier AI NextGO.Gray"));
        assert!(root_from_setting("%NIEZNANA%\\x", vars).is_none());
    }
}
