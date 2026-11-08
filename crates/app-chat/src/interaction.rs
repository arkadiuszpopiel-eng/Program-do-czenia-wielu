//! Sposób współpracy agentki (Jarvis): instrukcje rozmowy i przebiegu, bez zmiany uprawnień.

use agent_runtime_contract::RunSpec;
use personas_contract::Persona;

use crate::ChatEngine;

/// Wspólna wartość domyślna; persony mogą ją nadpisać.
const DEFAULT_KEY: &str = "agents.interaction_mode";

/// Instrukcje są dodatkiem do tożsamości i ról, nie zastępują promptu persony.
const JARVIS: &str = "Tryb współpracy Jarvis. Zachowujesz swoje imię, głos, charakter i rodzaj żeński. \
Mów krótko i naturalnie, tak aby odpowiedź nadawała się również do odsłuchu. Zachowaj długość \
wskazaną w poleceniu właściciela. Cechy charakteru i role opisuj, gdy o nie zapyta. Na zlecenie właściciela \
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

fn append(system: Option<String>, jarvis: bool, identity: Option<(&str, &str)>) -> Option<String> {
    if !jarvis {
        return system;
    }
    let mut prompt = match system {
        Some(s) if !s.is_empty() => format!("{s}\n\n{JARVIS}"),
        _ => JARVIS.to_owned(),
    };
    if let Some((name, instrumental)) = identity {
        prompt.push_str(&format!(
            "\nTożsamość tej agentki: imię „{name}”, forma „Jestem {instrumental}”."
        ));
    }
    Some(prompt)
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
        let enabled = self.jarvis_enabled(agent).await;
        let persona = enabled
            .then(|| {
                self.inner
                    .personas
                    .personas()
                    .into_iter()
                    .find(|p| p.id.as_str() == agent)
            })
            .flatten();
        append(system, enabled, persona.as_ref().map(identity))
    }

    pub(crate) async fn persona_interaction(
        &self,
        persona: &Persona,
        system: Option<String>,
    ) -> Option<String> {
        append(
            system,
            self.jarvis_enabled(persona.id.as_str()).await,
            Some(identity(persona)),
        )
    }

    /// Profil współpracy nowego przebiegu; zachowuje specyfikację ról, narzędzi i budżetów.
    pub(crate) async fn apply_interaction(&self, spec: &mut RunSpec) {
        spec.reply_style = self
            .persona_interaction(&spec.persona, spec.reply_style.take())
            .await;
    }
}

fn identity(persona: &Persona) -> (&str, &str) {
    (&persona.name, &persona.forms.instrumental)
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
        assert_eq!(
            append(system.clone(), false, Some(("Beta", "Betą"))),
            system
        );
        let jarvis = append(system, true, Some(("Beta", "Betą"))).unwrap_or_default();
        assert!(jarvis.starts_with("Jesteś Betą. Odpowiadaj szczegółowo.\n\n"));
        assert!(jarvis.contains("Tryb współpracy Jarvis"));
        assert!(jarvis.contains("W trybie Plan"));
        assert!(jarvis.ends_with("Tożsamość tej agentki: imię „Beta”, forma „Jestem Betą”."));
    }

    #[test]
    fn identity_uses_custom_forms_and_missing_identity_is_not_invented() {
        let mut persona = personas_contract::builtin_personas().remove(0);
        persona.name = "Luna".into();
        persona.forms.instrumental = "Luną".into();
        let custom = append(None, true, Some(identity(&persona))).unwrap_or_default();
        assert!(custom.ends_with("Tożsamość tej agentki: imię „Luna”, forma „Jestem Luną”."));
        assert_eq!(append(None, true, None), Some(JARVIS.to_owned()));
    }
}
