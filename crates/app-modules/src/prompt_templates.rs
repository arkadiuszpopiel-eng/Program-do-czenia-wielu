//! Szablony promptów (P4): lista w konfiguracji (`chat.prompt_templates`, tekst JSON, wspólna
//! dla maszyn), walidacja przy zapisie (nazwa, treść, zmienne `{{nazwa}}`, limity) i nadawanie
//! unikalnych identyfikatorów. Wypełnianie zmiennych robi interfejs przed wstawieniem tekstu.

use app_api::AppError;
use app_api::dto::PromptTemplate;

/// Klucz konfiguracji.
pub const KEY: &str = "chat.prompt_templates";
/// Najwięcej szablonów.
pub const MAX_TEMPLATES: usize = 100;
/// Najwięcej zmiennych w szablonie.
pub const MAX_VARIABLES: usize = 10;
const MAX_NAME: usize = 60;
const MAX_TEXT: usize = 8000;
const MAX_VARIABLE: usize = 30;

/// Lista z wartości konfiguracji (tekst JSON albo tablica); błędne dane = pusta lista.
pub fn parse(value: Option<&serde_json::Value>) -> Vec<PromptTemplate> {
    match value {
        Some(serde_json::Value::String(text)) => serde_json::from_str(text).unwrap_or_default(),
        Some(v @ serde_json::Value::Array(_)) => {
            serde_json::from_value(v.clone()).unwrap_or_default()
        }
        _ => Vec::new(),
    }
}

/// Zmienne `{{nazwa}}` w kolejności pierwszego wystąpienia (bez powtórzeń); `Err` — zmienna
/// pusta, za długa albo niedomknięta.
pub fn variables(text: &str) -> Result<Vec<String>, String> {
    let mut out: Vec<String> = Vec::new();
    let mut rest = text;
    while let Some(start) = rest.find("{{") {
        let after = &rest[start + 2..];
        let end = after
            .find("}}")
            .ok_or_else(|| "niedomknięta zmienna „{{”".to_owned())?;
        let name = after[..end].trim();
        let valid = !name.is_empty()
            && name.chars().count() <= MAX_VARIABLE
            && name
                .chars()
                .all(|c| c.is_alphanumeric() || c == '_' || c == ' ' || c == '-');
        if !valid {
            return Err(format!(
                "zła nazwa zmiennej „{name}” (litery, cyfry, spacja, _ i -; do {MAX_VARIABLE} znaków)"
            ));
        }
        if !out.iter().any(|v| v == name) {
            out.push(name.to_owned());
        }
        rest = &after[end + 2..];
    }
    Ok(out)
}

fn slug(name: &str, taken: &[String]) -> String {
    let folded: String = lib_quick::text::fold(name)
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();
    let base = folded
        .split('-')
        .filter(|p| !p.is_empty())
        .collect::<Vec<_>>()
        .join("-");
    let base = if base.is_empty() {
        "szablon".to_owned()
    } else {
        base
    };
    let mut id = base.clone();
    let mut n = 2;
    while taken.contains(&id) {
        id = format!("{base}-{n}");
        n += 1;
    }
    id
}

fn bad(what: String) -> AppError {
    AppError::invalid_field("templates", what)
}

/// Walidacja i normalizacja listy przed zapisem; wynik — wartość konfiguracji.
pub fn validate(templates: Vec<PromptTemplate>) -> Result<serde_json::Value, AppError> {
    if templates.len() > MAX_TEMPLATES {
        return Err(bad(format!("najwyżej {MAX_TEMPLATES} szablonów")));
    }
    let mut out: Vec<PromptTemplate> = Vec::with_capacity(templates.len());
    for mut t in templates {
        t.name = t.name.trim().to_owned();
        if t.name.is_empty() || t.name.chars().count() > MAX_NAME {
            return Err(bad(format!("nazwa szablonu od 1 do {MAX_NAME} znaków")));
        }
        t.text = t.text.trim().to_owned();
        if t.text.is_empty() || t.text.chars().count() > MAX_TEXT {
            return Err(bad(format!(
                "„{}”: treść od 1 do {MAX_TEXT} znaków",
                t.name
            )));
        }
        let vars = variables(&t.text).map_err(|e| bad(format!("„{}”: {e}", t.name)))?;
        if vars.len() > MAX_VARIABLES {
            return Err(bad(format!(
                "„{}”: najwyżej {MAX_VARIABLES} zmiennych",
                t.name
            )));
        }
        let taken: Vec<String> = out.iter().map(|o| o.id.clone()).collect();
        if t.id.trim().is_empty() || taken.contains(&t.id) {
            t.id = slug(&t.name, &taken);
        }
        out.push(t);
    }
    let text = serde_json::to_string(&out).map_err(|e| AppError::internal(e.to_string()))?;
    Ok(serde_json::Value::String(text))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn template(name: &str, text: &str) -> PromptTemplate {
        PromptTemplate {
            id: String::new(),
            name: name.into(),
            text: text.into(),
        }
    }

    #[test]
    fn variables_validation_and_ids() {
        assert_eq!(
            variables("Napisz e-mail do {{adresat}} o {{ temat }}; pozdrów {{adresat}}.").unwrap(),
            vec!["adresat".to_owned(), "temat".to_owned()]
        );
        assert!(variables("bez zmiennych").unwrap().is_empty());
        assert!(variables("{{otwarta").is_err());
        assert!(variables("{{}}").is_err());
        assert!(variables("{{zła<nazwa>}}").is_err());

        let value = validate(vec![
            template(" Streszczenie ", "Streść: {{tekst}}"),
            template("Streszczenie", "Krócej: {{tekst}}"),
        ])
        .unwrap();
        let list = parse(Some(&value));
        assert_eq!(list[0].name, "Streszczenie");
        assert_eq!(list[0].id, "streszczenie");
        assert_eq!(list[1].id, "streszczenie-2");
        assert!(validate(vec![template("", "x")]).is_err());
        assert!(validate(vec![template("a", " ")]).is_err());
        assert!(validate(vec![template("a", "{{x")]).is_err());
        let many: String = (0..11).map(|i| format!("{{{{v{i}}}}} ")).collect();
        assert!(validate(vec![template("a", &many)]).is_err());
        assert!(validate(vec![template("a", "x"); 101]).is_err());
    }
}
