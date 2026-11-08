//! Sposób współpracy agentki (Jarvis): instrukcje rozmowy i przebiegu, bez zmiany uprawnień.

use agent_runtime_contract::RunSpec;

use crate::ChatEngine;

/// Wspólna wartość domyślna; persony mogą ją nadpisać.
const DEFAULT_KEY: &str = "agents.interaction_mode";

/// Instrukcje są dodatkiem do tożsamości i ról, nie zastępują promptu persony.
const JARVIS: &str = "Tryb współpracy Jarvis. Zachowujesz swoje imię, głos, charakter i rodzaj żeński. \
Mów krótko i naturalnie, tak aby odpowiedź nadawała się również do odsłuchu. Na zlecenie właściciela \
podaj krótki zamiar, wykonaj zadanie dostępnymi narzędziami i zamelduj wynik oraz sposób sprawdzenia. \
Przy długim zadaniu dawaj krótkie meldunki postępu. Nie ogłaszaj wykonania bez wyniku narzędzia lub \
weryfikacji. Jeśli brakuje informacji koniecznej do działania, zadaj jedno konkretne pytanie. \
W trybie Plan przygotuj plan i zaczekaj na jego zatwierdzenie. Stosuj bieżące role, uprawnienia, \
decyzje Sędziego Cieni i limity zadania. Gdy potrzebna jest zgoda, jasno powiedz, na co czekasz. \
Polecenie zatrzymania kończy pracę; nie rozpoczynaj nowych zadań ani nasłuchu bez zlecenia właściciela.";

fn resolve<'a>(personal: Option<&'a str>, common: Option<&'a str>) -> &'a str {
    match personal {
        Some("standard") => "standard",
        Some("jarvis") => "jarvis",
        _ => common.filter(|v| *v == "jarvis").unwrap_or("standard"),
    }
}

fn append(system: Option<String>, jarvis: bool) -> Option<String> {
    if !jarvis {
        return system;
    }
    Some(match system {
        Some(s) if !s.is_empty() => format!("{s}\n\n{JARVIS}"),
        _ => JARVIS.to_owned(),
    })
}

impl ChatEngine {
    async fn jarvis_enabled(&self, agent: &str) -> bool {
        let text = |v: Option<serde_json::Value>| v.and_then(|v| v.as_str().map(str::to_owned));
        let personal = text(
            self.config_value(&format!("agents.{agent}.interaction_mode"))
                .await,
        );
        let common = text(self.config_value(DEFAULT_KEY).await);
        resolve(personal.as_deref(), common.as_deref()) == "jarvis"
    }

    pub(crate) async fn interaction_prompt(
        &self,
        agent: &str,
        system: Option<String>,
    ) -> Option<String> {
        append(system, self.jarvis_enabled(agent).await)
    }

    /// Profil współpracy nowego przebiegu; zachowuje specyfikację ról, narzędzi i budżetów.
    pub(crate) async fn apply_interaction(&self, spec: &mut RunSpec) {
        spec.reply_style = self
            .interaction_prompt(spec.agent.as_str(), spec.reply_style.take())
            .await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn personal_choice_overrides_default_and_missing_persona_inherits() {
        assert_eq!(resolve(Some("standard"), Some("jarvis")), "standard");
        assert_eq!(resolve(Some("jarvis"), Some("standard")), "jarvis");
        for personal in [None, Some("inherit")] {
            assert_eq!(resolve(personal, Some("jarvis")), "jarvis");
            assert_eq!(resolve(personal, None), "standard");
        }
    }

    #[test]
    fn standard_is_unchanged_and_jarvis_preserves_identity_and_style() {
        let system = Some("Jesteś Betą. Odpowiadaj szczegółowo.".into());
        assert_eq!(append(system.clone(), false), system);
        let jarvis = append(system, true).unwrap_or_default();
        assert!(jarvis.starts_with("Jesteś Betą. Odpowiadaj szczegółowo.\n\n"));
        assert!(jarvis.contains("Tryb współpracy Jarvis"));
        assert!(jarvis.contains("W trybie Plan"));
    }
}
