//! Współdzielone testy kontraktowe konfiguracji (feature `contract-tests`).
//! Ten sam zestaw uruchamiają `core-config-impl` i `core-config-fake`; rozjazd = błąd.
//!
//! Środowisko (`Harness`) musi mieć zarejestrowany schemat `fixture_schema()` pod prefiksem
//! `FIXTURE_PREFIX` (wartości domyślne = `fixture_defaults()`) i puste warstwy Shared/Machine.

use std::fmt::Display;

use core_bus_contract::{AgentId, SessionId};
use futures_util::{FutureExt, StreamExt};
use serde_json::json;

use crate::{
    ConfigChange, ConfigError, ConfigKey, ConfigLayer, ConfigStore, ConfigValue, ConfigWatch,
    MachineId, Origin, Scope,
};

/// Prefiks kluczy schematu testowego.
pub const FIXTURE_PREFIX: &str = "test";

/// JSON Schema poddrzewa `test.*` (z wartościami domyślnymi).
pub fn fixture_schema() -> serde_json::Value {
    json!({
        "type": "object",
        "properties": {
            "name": {"type": "string", "default": "alfa"},
            "level": {"type": "integer", "minimum": 0, "maximum": 10, "default": 3},
            "flag": {"type": "boolean"}
        },
        "additionalProperties": false
    })
}

/// Wartości domyślne zgodne z `fixture_schema()`.
pub fn fixture_defaults() -> Vec<(ConfigKey, ConfigValue)> {
    vec![
        (key("test.name"), json!("alfa")),
        (key("test.level"), json!(3)),
    ]
}

/// Środowisko testu: świeży magazyn i identyfikator bieżącej maszyny.
pub struct Harness<S> {
    /// Magazyn ze schematem testowym.
    pub store: S,
    /// Maszyna, której nakładkę magazyn czyta.
    pub machine: MachineId,
}

fn ok<T, E: Display>(r: Result<T, E>) -> T {
    r.unwrap_or_else(|e| panic!("{e}"))
}

fn key(s: &str) -> ConfigKey {
    ok(ConfigKey::new(s))
}

async fn get<S: ConfigStore>(s: &S, k: &str, scope: &Scope) -> Option<ConfigValue> {
    ok(s.get(&key(k), scope).await)
}

async fn put<S: ConfigStore>(
    s: &S,
    k: &str,
    v: Option<ConfigValue>,
    scope: &Scope,
    layer: &ConfigLayer,
) {
    ok(s.set(&key(k), v, scope, layer, Origin::User).await);
}

fn pending(w: &mut ConfigWatch) -> Option<ConfigChange> {
    match w.next().now_or_never() {
        Some(Some(change)) => Some(change),
        Some(None) => panic!("strumień watch zakończył się"),
        None => None,
    }
}

/// Wartości domyślne ze schematu są widoczne; nieznany klucz → `None`.
pub async fn defaults_are_visible<S: ConfigStore>(h: Harness<S>) {
    let g = Scope::Global;
    assert_eq!(get(&h.store, "test.name", &g).await, Some(json!("alfa")));
    assert_eq!(get(&h.store, "test.level", &g).await, Some(json!(3)));
    assert_eq!(get(&h.store, "test.flag", &g).await, None);
}

/// Machine > Shared > Default; usunięcie nadpisania odsłania niższą warstwę.
pub async fn layer_precedence<S: ConfigStore>(h: Harness<S>) {
    let (g, s) = (Scope::Global, &h.store);
    let machine = ConfigLayer::Machine(h.machine.clone());
    put(s, "test.level", Some(json!(5)), &g, &ConfigLayer::Shared).await;
    assert_eq!(get(s, "test.level", &g).await, Some(json!(5)));
    put(s, "test.level", Some(json!(7)), &g, &machine).await;
    put(s, "test.level", Some(json!(6)), &g, &ConfigLayer::Shared).await;
    assert_eq!(get(s, "test.level", &g).await, Some(json!(7)));
    put(s, "test.level", None, &g, &machine).await;
    assert_eq!(get(s, "test.level", &g).await, Some(json!(6)));
    put(s, "test.level", None, &g, &ConfigLayer::Shared).await;
    assert_eq!(get(s, "test.level", &g).await, Some(json!(3)));
}

/// Sesja/agentka > maszyna > wspólna > domyślna; zakresy są od siebie odseparowane.
pub async fn scope_precedence<S: ConfigStore>(h: Harness<S>) {
    let (g, s) = (Scope::Global, &h.store);
    let s1 = Scope::Session(SessionId::from("s1"));
    let s2 = Scope::Session(SessionId::from("s2"));
    let agent = Scope::Agent(AgentId::from("ala"));
    let machine = ConfigLayer::Machine(h.machine.clone());
    put(s, "test.level", Some(json!(7)), &g, &machine).await;
    put(s, "test.level", Some(json!(9)), &s1, &ConfigLayer::Shared).await;
    put(s, "test.level", Some(json!(1)), &agent, &machine).await;
    assert_eq!(get(s, "test.level", &s1).await, Some(json!(9)));
    assert_eq!(get(s, "test.level", &s2).await, Some(json!(7)));
    assert_eq!(get(s, "test.level", &agent).await, Some(json!(1)));
    assert_eq!(get(s, "test.level", &g).await, Some(json!(7)));
    assert_eq!(get(s, "test.name", &s1).await, Some(json!("alfa")));
    put(s, "test.level", None, &s1, &ConfigLayer::Shared).await;
    assert_eq!(get(s, "test.level", &s1).await, Some(json!(7)));
}

/// `watch(prefix)` dostaje tylko zmiany wartości wynikowej kluczy pod prefiksem.
pub async fn watch_filters_by_prefix<S: ConfigStore>(h: Harness<S>) {
    let (g, s) = (Scope::Global, &h.store);
    let machine = ConfigLayer::Machine(h.machine.clone());
    let mut test = s.watch("test");
    let mut other = s.watch("other");
    let mut all = s.watch("");
    put(
        s,
        "test.name",
        Some(json!("zeta")),
        &g,
        &ConfigLayer::Shared,
    )
    .await;
    let change = pending(&mut test).unwrap_or_else(|| panic!("brak zmiany"));
    assert_eq!(change.key, key("test.name"));
    assert_eq!(change.scope, Scope::Global);
    assert_eq!(change.layer, ConfigLayer::Shared);
    assert_eq!(change.old, Some(json!("alfa")));
    assert_eq!(change.new, Some(json!("zeta")));
    assert_eq!(change.origin, Origin::User);
    assert!(pending(&mut all).is_some());
    assert!(
        pending(&mut other).is_none(),
        "inny prefiks nie dostaje zmian"
    );
    put(
        s,
        "test.name",
        Some(json!("zeta")),
        &g,
        &ConfigLayer::Shared,
    )
    .await;
    assert!(
        pending(&mut test).is_none(),
        "ta sama wartość to nie zmiana"
    );
    put(s, "test.name", Some(json!("omega")), &g, &machine).await;
    let change = pending(&mut test).unwrap_or_else(|| panic!("brak zmiany"));
    assert_eq!(
        (change.old, change.new),
        (Some(json!("zeta")), Some(json!("omega")))
    );
    put(s, "test.name", Some(json!("psi")), &g, &ConfigLayer::Shared).await;
    assert!(pending(&mut test).is_none(), "zasłonięte przez maszynę");
    put(s, "test.name", None, &g, &machine).await;
    let change = pending(&mut test).unwrap_or_else(|| panic!("brak zmiany"));
    assert_eq!(
        (change.old, change.new),
        (Some(json!("omega")), Some(json!("psi")))
    );
}

/// Klucze `kernel.*` zmienia wyłącznie Broker (100% pozostałych `Origin` odrzuconych).
pub async fn kernel_keys_only_via_broker<S: ConfigStore>(h: Harness<S>) {
    let (g, s) = (Scope::Global, &h.store);
    let k = key("kernel.egress.allow_all");
    for origin in [
        Origin::User,
        Origin::Module("voice-stt".into()),
        Origin::Improver,
        Origin::Import,
    ] {
        let res = s
            .set(&k, Some(json!(true)), &g, &ConfigLayer::Shared, origin)
            .await;
        assert_eq!(res, Err(ConfigError::KernelPolicy(k.clone())));
    }
    assert_eq!(
        ok(s.get(&k, &g).await),
        None,
        "odrzucony zapis nie zostawia śladu"
    );
    let broker = s
        .set(
            &k,
            Some(json!(false)),
            &g,
            &ConfigLayer::Shared,
            Origin::Broker,
        )
        .await;
    assert_eq!(broker, Ok(()));
    assert_eq!(ok(s.get(&k, &g).await), Some(json!(false)));
}

/// Zapis do warstwy Default, do nakładki cudzej maszyny, `null` i obiektów → błąd.
pub async fn invalid_writes_are_rejected<S: ConfigStore>(h: Harness<S>) {
    let (g, s) = (Scope::Global, &h.store);
    let k = key("test.name");
    let shared = ConfigLayer::Shared;
    let foreign = ConfigLayer::Machine(MachineId::new("inna-maszyna"));
    let cases = [
        (ConfigLayer::Default, json!("x"), "persist"),
        (foreign, json!("x"), "persist"),
        (shared.clone(), serde_json::Value::Null, "schema"),
        (shared, json!({"a": 1}), "schema"),
    ];
    for (layer, value, expected) in cases {
        let res = s.set(&k, Some(value), &g, &layer, Origin::User).await;
        let kind = match res {
            Err(ConfigError::Persist(_)) => "persist",
            Err(ConfigError::SchemaViolation { .. }) => "schema",
            other => panic!("nieoczekiwany wynik dla {layer:?}: {other:?}"),
        };
        assert_eq!(kind, expected, "{layer:?}");
    }
    assert_eq!(get(s, "test.name", &g).await, Some(json!("alfa")));
}

/// Uruchamia cały zestaw; `factory` daje świeże środowisko dla każdego przypadku.
pub async fn run_all<S: ConfigStore, F: Fn() -> Harness<S>>(factory: F) {
    defaults_are_visible(factory()).await;
    layer_precedence(factory()).await;
    scope_precedence(factory()).await;
    watch_filters_by_prefix(factory()).await;
    kernel_keys_only_via_broker(factory()).await;
    invalid_writes_are_rejected(factory()).await;
}
