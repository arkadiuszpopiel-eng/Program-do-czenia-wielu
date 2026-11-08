//! Tabela tras z konfiguracji (`router.*` w `config/*.toml`, SPEC router): klasy
//! (`router.class.<klasa>.prefer = ["dostawca:model", …]`), terminy pierwszego zdarzenia,
//! obwód, Mówczyni/Myślicielka. Nadpisania trafiają do wszystkich rdzeni Routera; zmiana pliku
//! jest stosowana na żywo (obserwacja prefiksu `router.`).

use std::sync::Arc;

use core_config_contract::{ConfigKey, ConfigStore, Scope};
use core_config_impl::FileConfigStore;
use futures_util::StreamExt;
use router_contract::ALL_CLASSES;
use serde_json::Value;

use super::Routers;

async fn value(config: &FileConfigStore, key: &str) -> Option<Value> {
    let key = ConfigKey::new(key).ok()?;
    config.get(&key, &Scope::Global).await.ok().flatten()
}

fn class_name(class: router_contract::TaskClass) -> Option<String> {
    serde_json::to_value(class)
        .ok()
        .and_then(|v| v.as_str().map(str::to_owned))
}

/// Sekcja `[router]` w TOML z płaskich kluczy konfiguracji (`None` = brak nadpisań).
pub(crate) async fn overrides(config: &FileConfigStore) -> Option<String> {
    let mut root = toml::Table::new();
    let mut classes = toml::Table::new();
    let mut deadlines = toml::Table::new();
    for class in ALL_CLASSES {
        let Some(name) = class_name(class) else {
            continue;
        };
        if let Some(Value::Array(items)) =
            value(config, &format!("router.class.{name}.prefer")).await
        {
            let prefer: Vec<toml::Value> = items
                .iter()
                .filter_map(|v| v.as_str().map(|s| toml::Value::String(s.to_owned())))
                .collect();
            let mut table = toml::Table::new();
            table.insert("prefer".into(), toml::Value::Array(prefer));
            classes.insert(name.clone(), toml::Value::Table(table));
        }
        if let Some(Value::String(d)) = value(config, &format!("router.deadline.{name}")).await {
            deadlines.insert(name, toml::Value::String(d));
        }
    }
    let mut breaker = toml::Table::new();
    if let Some(n) = value(config, "router.breaker.failures")
        .await
        .and_then(|v| v.as_i64())
    {
        breaker.insert("failures".into(), toml::Value::Integer(n));
    }
    for key in ["window", "cooldown"] {
        if let Some(Value::String(s)) = value(config, &format!("router.breaker.{key}")).await {
            breaker.insert(key.into(), toml::Value::String(s));
        }
    }
    for key in ["speaker", "thinker"] {
        if let Some(Value::String(s)) = value(config, &format!("router.{key}")).await {
            root.insert(key.into(), toml::Value::String(s));
        }
    }
    for (name, table) in [
        ("class", classes),
        ("deadline", deadlines),
        ("breaker", breaker),
    ] {
        if !table.is_empty() {
            root.insert(name.into(), toml::Value::Table(table));
        }
    }
    (!root.is_empty()).then(|| root.to_string())
}

/// Nadpisania obejmują rdzenie z trasami API; lokalny zostaje przy polityce automatycznej
/// (jedyny kandydat to model lokalny).
fn set(routers: &Routers, text: Option<String>) {
    for core in [&routers.hybrid, &routers.cloud] {
        if let Err(e) = core.set_overrides_toml(text.clone()) {
            tracing::warn!(error = %e, "niepoprawna konfiguracja [router] — polityka automatyczna");
            let _ = core.set_overrides_toml(None);
        }
    }
}

/// Stosuje nadpisania i obserwuje zmiany `router.*`.
pub async fn apply(config: &Arc<FileConfigStore>, routers: &Routers) {
    set(routers, overrides(config).await);
    let mut watch = config.watch("router.");
    let config = config.clone();
    let routers = routers.clone();
    tokio::spawn(async move {
        while watch.next().await.is_some() {
            set(&routers, overrides(&config).await);
        }
    });
}
