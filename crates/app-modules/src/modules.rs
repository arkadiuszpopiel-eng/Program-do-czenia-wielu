//! Ustawienia → Moduły: lista modułów z manifestów i rejestru (stan, budżety, zdolności,
//! zależności) oraz wyłączanie modułów opcjonalnych. Wyłączenie działa od następnego
//! uruchomienia — usługa nie jest wtedy budowana, a funkcje zależne odpowiadają „moduł
//! niepodłączony”. Moduły Jądra (Broker, watchdog, audyt, aktualizacje, `core-*`) są chronione
//! (reguła twarda `AGENTS.md`), a lista opcjonalnych jest zamknięta i sprawdzona testem rdzenia.

use std::collections::BTreeSet;

use app_api::dto::{ModuleRow, ModulesView};
use core_registry_contract::{
    Isolation, Lifecycle, ModuleKind, ModuleManifest, ModuleState, ModuleStatus,
};

/// Klucz konfiguracji (per maszyna): lista wyłączonych modułów jako tekst JSON.
pub const DISABLED_KEY: &str = "modules.disabled";

/// Moduły, których brak program obsługuje (port odpowiada „niepodłączony”, rozmowa działa dalej).
pub const OPTIONAL: [&str; 5] = [
    "providers-local",
    "voice-audio",
    "voice-tts",
    "memory-consolidation",
    "transfer",
];

/// Moduły Jądra — nigdy do wyłączenia ani zmiany z interfejsu.
pub fn is_protected(id: &str) -> bool {
    id.starts_with("core-")
        || matches!(
            id,
            "safety-broker"
                | "broker-ui"
                | "watchdog"
                | "updater"
                | "risk-classifier"
                | "compliance"
        )
}

/// Czy moduł wolno wyłączyć.
pub fn can_disable(id: &str) -> bool {
    OPTIONAL.contains(&id) && !is_protected(id)
}

/// Lista wyłączonych z wartości konfiguracji (tekst JSON albo tablica); tylko moduły opcjonalne.
pub fn parse_disabled(value: Option<&serde_json::Value>) -> BTreeSet<String> {
    let list: Vec<String> = match value {
        Some(serde_json::Value::String(text)) => serde_json::from_str(text).unwrap_or_default(),
        Some(v @ serde_json::Value::Array(_)) => {
            serde_json::from_value(v.clone()).unwrap_or_default()
        }
        _ => Vec::new(),
    };
    list.into_iter().filter(|id| can_disable(id)).collect()
}

/// Wartość konfiguracji dla listy wyłączonych.
pub fn disabled_value(set: &BTreeSet<String>) -> serde_json::Value {
    serde_json::Value::String(
        serde_json::to_string(&set.iter().collect::<Vec<_>>()).unwrap_or_default(),
    )
}

fn kind(k: ModuleKind) -> &'static str {
    match k {
        ModuleKind::Service => "service",
        ModuleKind::Tool => "tool",
        ModuleKind::Provider => "provider",
        ModuleKind::VoiceEngine => "voice-engine",
        ModuleKind::AgentPack => "agent-pack",
        ModuleKind::UiPanel => "ui-panel",
    }
}

fn lifecycle(l: Lifecycle) -> &'static str {
    match l {
        Lifecycle::Always => "always",
        Lifecycle::Lazy => "lazy",
        Lifecycle::OnDemand => "on-demand",
    }
}

fn isolation(i: Isolation) -> &'static str {
    match i {
        Isolation::InProc => "inproc",
        Isolation::Process => "process",
        Isolation::Wasm => "wasm",
    }
}

fn state(s: Option<&ModuleState>) -> (&'static str, Option<String>) {
    match s {
        Some(ModuleState::Disabled) => ("disabled", None),
        None | Some(ModuleState::Unloaded) => ("unloaded", None),
        Some(ModuleState::Loading) => ("loading", None),
        Some(ModuleState::Ready) => ("ready", None),
        Some(ModuleState::Degraded { reason }) => ("degraded", Some(reason.clone())),
        Some(ModuleState::Failed { reason, .. }) => ("failed", Some(reason.clone())),
    }
}

/// Widok listy: manifesty (kolejność po `id`), stany z rejestru, wyłączone w ustawieniach
/// i pominięte przy starcie.
pub fn view(
    manifests: &[ModuleManifest],
    statuses: &[ModuleStatus],
    disabled: &BTreeSet<String>,
    skipped: &BTreeSet<String>,
) -> ModulesView {
    let mut modules: Vec<ModuleRow> = manifests
        .iter()
        .map(|m| {
            let id = m.id.to_string();
            let status = statuses.iter().find(|s| s.id == m.id).map(|s| &s.state);
            let (mut st, mut detail) = state(status);
            if skipped.contains(&id) {
                st = "disabled";
                detail = Some("wyłączony w ustawieniach".into());
            }
            ModuleRow {
                version: m.version.to_string(),
                kind: kind(m.kind).into(),
                lifecycle: lifecycle(m.lifecycle).into(),
                isolation: isolation(m.isolation).into(),
                state: st.into(),
                detail,
                ram_mb: m.budget.ram_mb,
                cpu_pct: m.budget.cpu_pct,
                vram_mb: m.budget.vram_mb,
                capabilities: m.capabilities.iter().map(ToString::to_string).collect(),
                provides: m.provides.iter().map(ToString::to_string).collect(),
                requires: m.requires.iter().map(ToString::to_string).collect(),
                protected: is_protected(&id),
                can_disable: can_disable(&id),
                disabled: disabled.contains(&id),
                skipped: skipped.contains(&id),
                id,
            }
        })
        .collect();
    modules.sort_by(|a, b| a.id.cmp(&b.id));
    let restart_needed = modules.iter().any(|m| m.disabled != m.skipped);
    let ram_total_mb = modules
        .iter()
        .filter(|m| !m.disabled)
        .map(|m| m.ram_mb)
        .sum();
    ModulesView {
        modules,
        restart_needed,
        ram_total_mb,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manifest(id: &str, ram: u32) -> ModuleManifest {
        ModuleManifest::parse_toml(&format!(
            "id = \"{id}\"\nversion = \"0.1.0\"\nkind = \"service\"\nprovides = []\nrequires = []\ncapabilities = []\nlifecycle = \"lazy\"\nisolation = \"inproc\"\n[budget]\nram_mb = {ram}\ncpu_pct = 5\n[health]\ncheck = \"ping\"\ninterval_s = 60\n"
        ))
        .unwrap()
    }

    #[test]
    fn protected_optional_and_parsing() {
        assert!(is_protected("safety-broker") && is_protected("core-config"));
        assert!(!can_disable("safety-broker") && can_disable("voice-tts"));
        assert!(!can_disable("sessions"), "moduł niezbędny");
        let v = serde_json::json!("[\"voice-tts\", \"safety-broker\", \"nie-ma\"]");
        let set = parse_disabled(Some(&v));
        assert_eq!(set.iter().collect::<Vec<_>>(), vec!["voice-tts"]);
        assert_eq!(parse_disabled(Some(&disabled_value(&set))), set);
        assert!(parse_disabled(None).is_empty());
    }

    #[test]
    fn view_marks_restart_and_totals() {
        let ms = [manifest("voice-tts", 300), manifest("safety-broker", 50)];
        let disabled: BTreeSet<String> = ["voice-tts".to_owned()].into();
        let v = view(&ms, &[], &disabled, &BTreeSet::new());
        assert!(v.restart_needed);
        assert_eq!(v.ram_total_mb, 50);
        let broker = v.modules.iter().find(|m| m.id == "safety-broker").unwrap();
        assert!(broker.protected && !broker.can_disable);
        let after = view(&ms, &[], &disabled, &disabled);
        assert!(!after.restart_needed);
        assert_eq!(after.modules[1].state, "disabled");
    }
}
