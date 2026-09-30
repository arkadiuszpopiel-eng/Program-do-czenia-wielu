//! Warstwy, zakresy i rozwiązywanie wartości wynikowej.

use core_bus_contract::{AgentId, SessionId};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Identyfikator maszyny (z `device-profile`); nazwa pliku nakładki `config/machine/<id>.toml`.
#[derive(
    Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(transparent)]
pub struct MachineId(pub String);

impl MachineId {
    /// Tworzy identyfikator.
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }
}

/// Zakres ustawienia (SPEC: sesja/agentka nadpisują maszynę i warstwę wspólną).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "scope", content = "id", rename_all = "kebab-case")]
pub enum Scope {
    /// Cała aplikacja.
    Global,
    /// Jedna sesja.
    Session(SessionId),
    /// Jedna agentka.
    Agent(AgentId),
}

/// Warstwa pliku konfiguracji (PLAN §3.5). Wyższy `precedence` wygrywa.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "layer", content = "id", rename_all = "kebab-case")]
pub enum ConfigLayer {
    /// Wartość domyślna z manifestu/schematu modułu.
    Default,
    /// Warstwa wspólna dla wszystkich maszyn (agentki, obsada, reguły, uprawnienia).
    Shared,
    /// Nakładka per maszyna (urządzenia audio, profil głosu, moduły rezydentne, limity).
    Machine(MachineId),
}

impl ConfigLayer {
    /// Priorytet warstwy: `Default` (0) < `Shared` (1) < `Machine` (2).
    pub fn precedence(&self) -> u8 {
        match self {
            ConfigLayer::Default => 0,
            ConfigLayer::Shared => 1,
            ConfigLayer::Machine(_) => 2,
        }
    }
}

/// Wartość wynikowa: z podanych warstw wybiera tę o najwyższym priorytecie.
/// Przy remisie (dwie nakładki maszynowe) wygrywa późniejsza na liście.
pub fn resolve<'a, V>(layers: impl IntoIterator<Item = (&'a ConfigLayer, &'a V)>) -> Option<&'a V> {
    let mut best: Option<(u8, &'a V)> = None;
    for (layer, value) in layers {
        let p = layer.precedence();
        if best.is_none_or(|(bp, _)| p >= bp) {
            best = Some((p, value));
        }
    }
    best.map(|(_, v)| v)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn machine_overrides_shared_overrides_default() {
        let m = ConfigLayer::Machine(MachineId::new("desktop"));
        let layers = [
            (&ConfigLayer::Default, &"d"),
            (&m, &"m"),
            (&ConfigLayer::Shared, &"s"),
        ];
        assert_eq!(resolve(layers.iter().copied()), Some(&"m"));
        let two = [(&ConfigLayer::Shared, &"s"), (&ConfigLayer::Default, &"d")];
        assert_eq!(resolve(two.iter().copied()), Some(&"s"));
        assert_eq!(resolve(std::iter::empty::<(&ConfigLayer, &&str)>()), None);
    }

    #[test]
    fn scope_serializes_tagged() {
        let s = Scope::Session(SessionId::from("s1"));
        let json = serde_json::to_value(&s).unwrap();
        assert_eq!(json, serde_json::json!({"scope": "session", "id": "s1"}));
        let back: Scope = serde_json::from_value(json).unwrap();
        assert_eq!(back, s);
    }
}
