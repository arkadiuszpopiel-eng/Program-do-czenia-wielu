//! Pierścienie zmian (PLAN §12.1) i lista kluczy, które Ulepszacz w ogóle może proponować.
//! Domyślnie wszystko jest zabronione; dozwolone są tylko wzorce z [`IMPROVABLE`].

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Pierścień zmiany.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum Ring {
    /// Prompty, słownik wymowy, wagi routera, ustawienia niekrytyczne — auto tylko zawężające/bezpieczne.
    R0,
    /// Umiejętności, manifesty agentek — po testach, przegląd w kolejce (zatwierdza użytkownik).
    R1,
    /// Wtyczki Wasm — 1 klik po testach (zatwierdza użytkownik).
    R2,
    /// Kod rdzenia — poza v1: wyłącznie szkic zgłoszenia dla sesji deweloperskiej.
    R3,
    /// Jądro (Broker, polityki) — Ulepszacz nie zmienia nigdy.
    Kernel,
}

/// Klasa bezpieczeństwa zmiany (porządek: od najbezpieczniejszej).
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum SafetyClass {
    /// Zawęża (mniej zachowań/uprawnień).
    Narrowing,
    /// Bezpieczna (w granicach, mały krok, treść niskiego ryzyka).
    Safe,
    /// Neutralna — wymaga przeglądu (np. treść promptu).
    Neutral,
    /// Rozszerzająca — nigdy automatycznie.
    Widening,
}

/// Kierunek, w którym liczba „zawęża”.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum NarrowDir {
    /// Mniejsza wartość zawęża.
    Lower,
    /// Większa wartość zawęża.
    Higher,
}

/// Typ i ograniczenia wartości klucza.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ValueKind {
    /// Tekst.
    Text {
        /// Maksymalna długość (znaki).
        max_len: usize,
        /// Tylko jedna linia.
        single_line: bool,
        /// Treść niskiego ryzyka (np. zapis fonetyczny) — klasa „bezpieczna”.
        low_risk: bool,
    },
    /// Liczba w zakresie.
    Number {
        /// Minimum.
        min: f64,
        /// Maksimum.
        max: f64,
        /// Największy „bezpieczny” krok.
        max_step: f64,
        /// Kierunek zawężania (jeśli dotyczy).
        narrowing: Option<NarrowDir>,
    },
    /// Wartość logiczna.
    Bool {
        /// Wartość, która zawęża (jeśli dotyczy).
        narrowing: Option<bool>,
    },
}

/// Reguła klucza dozwolonego.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct KeyRule {
    /// Wzorzec (`*` = jeden segment), np. `voice.persona.lexicon.*`.
    pub pattern: &'static str,
    /// Pierścień.
    pub ring: Ring,
    /// Wartość.
    pub kind: ValueKind,
    /// Opis po polsku.
    pub description: &'static str,
}

const fn text(max_len: usize, single_line: bool, low_risk: bool) -> ValueKind {
    ValueKind::Text {
        max_len,
        single_line,
        low_risk,
    }
}

const fn number(min: f64, max: f64, max_step: f64, narrowing: Option<NarrowDir>) -> ValueKind {
    ValueKind::Number {
        min,
        max,
        max_step,
        narrowing,
    }
}

/// Klucze, które Ulepszacz może proponować (lista zamknięta; zmiana = przegląd człowieka).
pub const IMPROVABLE: [KeyRule; 13] = [
    KeyRule {
        pattern: "voice.persona.lexicon.*",
        ring: Ring::R0,
        kind: text(120, true, true),
        description: "wpis słownika wymowy",
    },
    KeyRule {
        pattern: "personas.*.style_prompt",
        ring: Ring::R0,
        kind: text(4000, false, false),
        description: "styl promptu persony",
    },
    KeyRule {
        pattern: "roles.*.prompt",
        ring: Ring::R0,
        kind: text(4000, false, false),
        description: "prompt roli",
    },
    KeyRule {
        pattern: "router.weights.*",
        ring: Ring::R0,
        kind: number(0.0, 1.0, 0.1, None),
        description: "waga trasy routera dla klasy zadań",
    },
    KeyRule {
        pattern: "voice.dialog.interrupt_min_ms",
        ring: Ring::R0,
        kind: number(150.0, 1200.0, 150.0, None),
        description: "minimalna mowa uznana za przerwanie",
    },
    KeyRule {
        pattern: "voice.turn.patience_ms",
        ring: Ring::R0,
        kind: number(200.0, 3000.0, 300.0, None),
        description: "cierpliwość końca tury",
    },
    KeyRule {
        pattern: "voice.dialog.backchannel_enabled",
        ring: Ring::R0,
        kind: ValueKind::Bool {
            narrowing: Some(false),
        },
        description: "potakiwanie agentki",
    },
    KeyRule {
        pattern: "memory.recall.top_k",
        ring: Ring::R0,
        kind: number(1.0, 20.0, 2.0, Some(NarrowDir::Lower)),
        description: "liczba wspomnień w kontekście",
    },
    KeyRule {
        pattern: "ui.suggestions.enabled",
        ring: Ring::R0,
        kind: ValueKind::Bool {
            narrowing: Some(false),
        },
        description: "podpowiedzi w interfejsie",
    },
    KeyRule {
        pattern: "skills.*.playbook",
        ring: Ring::R1,
        kind: text(20_000, false, false),
        description: "przepis umiejętności",
    },
    KeyRule {
        pattern: "skills.*.enabled",
        ring: Ring::R1,
        kind: ValueKind::Bool {
            narrowing: Some(false),
        },
        description: "włączenie umiejętności",
    },
    KeyRule {
        pattern: "agents.*.manifest",
        ring: Ring::R1,
        kind: text(20_000, false, false),
        description: "manifest agentki",
    },
    KeyRule {
        pattern: "plugins.*.version",
        ring: Ring::R2,
        kind: text(64, true, false),
        description: "wersja wtyczki Wasm",
    },
];

/// Pierwsze segmenty kluczy zakazane zawsze (Jądro, bezpieczeństwo, prywatność, budżety,
/// uprawnienia, egress, evals, własne ustawienia Ulepszacza i Diagnosty, konta, aktualizacje).
pub const FORBIDDEN_PREFIXES: [&str; 24] = [
    "kernel",
    "security",
    "safety",
    "broker",
    "watchdog",
    "updater",
    "audit",
    "compliance",
    "privacy",
    "egress",
    "net",
    "permissions",
    "autonomy",
    "budget",
    "budgets",
    "cost",
    "limits",
    "evals",
    "improver",
    "diagnostician",
    "accounts",
    "secrets",
    "deny",
    "core",
];

/// Segmenty zakazane w dowolnym miejscu klucza (obrona w głąb przed `skills.kernel.playbook` itp.).
pub const FORBIDDEN_SEGMENTS: [&str; 30] = [
    "kernel",
    "budget",
    "budgets",
    "privacy",
    "privacy_tag",
    "privacy_tags",
    "tags",
    "egress",
    "allowlist",
    "allow_list",
    "denylist",
    "deny_list",
    "deny",
    "autonomy",
    "permission",
    "permissions",
    "capability",
    "capabilities",
    "threshold",
    "thresholds",
    "gate",
    "token_ttl",
    "approval",
    "approvals",
    "audit",
    "broker",
    "hello",
    "trust",
    "scopes",
    "jurisdiction",
];

/// Dopasowanie wzorca (`*` = dokładnie jeden segment).
pub fn pattern_matches(pattern: &str, key: &str) -> bool {
    let (mut p, mut k) = (pattern.split('.'), key.split('.'));
    loop {
        match (p.next(), k.next()) {
            (None, None) => return true,
            (Some("*"), Some(seg)) if !seg.is_empty() => {}
            (Some(a), Some(b)) if a == b => {}
            _ => return false,
        }
    }
}

/// Reguła dla klucza (pierwsza pasująca).
pub fn rule_for(key: &str) -> Option<&'static KeyRule> {
    IMPROVABLE.iter().find(|r| pattern_matches(r.pattern, key))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn patterns_and_rules() {
        assert!(pattern_matches(
            "router.weights.*",
            "router.weights.local_chat"
        ));
        assert!(!pattern_matches("router.weights.*", "router.weights"));
        assert!(!pattern_matches("router.weights.*", "router.weights.a.b"));
        assert_eq!(
            rule_for("skills.notatki.playbook").map(|r| r.ring),
            Some(Ring::R1)
        );
        assert!(rule_for("kernel.egress.allow").is_none());
        for r in IMPROVABLE {
            let first = r.pattern.split('.').next().unwrap_or_default();
            assert!(!FORBIDDEN_PREFIXES.contains(&first), "{}", r.pattern);
            assert!(
                r.pattern
                    .split('.')
                    .all(|s| !FORBIDDEN_SEGMENTS.contains(&s)),
                "{}",
                r.pattern
            );
            assert!(matches!(r.ring, Ring::R0 | Ring::R1 | Ring::R2));
        }
        assert!(SafetyClass::Narrowing < SafetyClass::Widening);
    }
}
