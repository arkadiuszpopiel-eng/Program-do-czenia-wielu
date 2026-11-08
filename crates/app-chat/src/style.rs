//! Styl odpowiedzi i temperatura (P4): ustawienia `chat.reply_style` i `chat.temperature`
//! z Ustawienia → Ogólne. Styl to jedno zdanie dopisane do promptu systemowego agentki,
//! temperatura trafia do parametrów żądania (dostawca pomija ją u modeli, które jej nie przyjmują —
//! `caps.sampling` w adapterach). „auto” = bez zmian.

/// Klucz stylu odpowiedzi.
pub const STYLE_KEY: &str = "chat.reply_style";
/// Klucz temperatury.
pub const TEMPERATURE_KEY: &str = "chat.temperature";

/// Zdanie do promptu systemowego dla stylu (`None` — „auto” albo nieznany).
pub fn style_line(code: &str) -> Option<&'static str> {
    Some(match code {
        "concise" => {
            "Styl odpowiedzi wybrany przez użytkownika: zwięźle — najważniejsze w kilku zdaniach, bez wstępów i powtórzeń."
        }
        "detailed" => {
            "Styl odpowiedzi wybrany przez użytkownika: szczegółowo — wyjaśniaj krok po kroku, z przykładami."
        }
        "simple" => {
            "Styl odpowiedzi wybrany przez użytkownika: prostym językiem — krótkie zdania, bez żargonu; trudne słowa objaśniaj."
        }
        "formal" => {
            "Styl odpowiedzi wybrany przez użytkownika: formalnie — rzeczowo i uprzejmie, jak w korespondencji służbowej."
        }
        _ => return None,
    })
}

/// Temperatura dla ustawienia (`None` — „auto”, wartość domyślna modelu).
pub fn temperature(code: &str) -> Option<f32> {
    match code {
        "precise" => Some(0.2),
        "balanced" => Some(0.7),
        "creative" => Some(1.0),
        _ => None,
    }
}

/// Prompt systemowy z dopisanym stylem.
pub fn with_style(system: Option<String>, code: &str) -> Option<String> {
    match (system, style_line(code)) {
        (Some(s), Some(line)) => Some(format!("{s}\n\n{line}")),
        (None, Some(line)) => Some(line.to_owned()),
        (s, None) => s,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn styles_and_temperatures() {
        assert_eq!(
            with_style(Some("Jesteś Alfą.".into()), "auto").as_deref(),
            Some("Jesteś Alfą.")
        );
        let concise = with_style(Some("Jesteś Alfą.".into()), "concise").unwrap_or_default();
        assert!(concise.starts_with("Jesteś Alfą.\n\n") && concise.contains("zwięźle"));
        assert!(with_style(None, "formal").is_some_and(|s| s.contains("formalnie")));
        assert_eq!(with_style(None, "nieznany"), None);
        assert_eq!(temperature("precise"), Some(0.2));
        assert_eq!(temperature("auto"), None);
        for code in ["precise", "balanced", "creative"] {
            assert!(temperature(code).is_some_and(|t| (0.0..=2.0).contains(&t)));
        }
    }
}
