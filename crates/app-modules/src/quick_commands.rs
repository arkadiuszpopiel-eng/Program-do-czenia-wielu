//! Ustawienia → Polecenia bez modelu: własne polecenia użytkownika (lista w konfiguracji jako
//! tekst JSON), katalog wbudowanych i tester „co stanie się z tą wiadomością”. Walidacja przy
//! zapisie: limity długości i liczby, tryb, wyrażenia regularne kompilowane z limitem rozmiaru.

use app_api::AppError;
use app_api::dto::{
    BuiltinCommand, CustomCommand, QuickCommandsView, QuickMatchMode, QuickSource, QuickTest,
};
use lib_quick::custom::{self, CustomRef, Mode};

/// Klucz konfiguracji (wspólny dla maszyn): lista własnych poleceń jako tekst JSON.
pub const KEY: &str = lib_quick::keys::CUSTOM;
/// Najwięcej poleceń.
pub const MAX_COMMANDS: usize = 50;
/// Najwięcej fraz w poleceniu.
pub const MAX_PHRASES: usize = 10;
const MAX_PHRASE: usize = 200;
const MAX_NAME: usize = 60;
const MAX_REPLY: usize = 2000;

/// Tryb biblioteki dla trybu DTO.
pub fn mode(m: QuickMatchMode) -> Mode {
    match m {
        QuickMatchMode::Exact => Mode::Exact,
        QuickMatchMode::Starts => Mode::Starts,
        QuickMatchMode::Contains => Mode::Contains,
        QuickMatchMode::Regex => Mode::Regex,
    }
}

/// Lista z wartości konfiguracji (tekst JSON albo tablica); błędne dane = pusta lista.
pub fn parse(value: Option<&serde_json::Value>) -> Vec<CustomCommand> {
    match value {
        Some(serde_json::Value::String(text)) => serde_json::from_str(text).unwrap_or_default(),
        Some(v @ serde_json::Value::Array(_)) => {
            serde_json::from_value(v.clone()).unwrap_or_default()
        }
        _ => Vec::new(),
    }
}

/// Pierwsze pasujące własne polecenie: nazwa i odpowiedź.
pub fn find(commands: &[CustomCommand], message: &str) -> Option<(String, String)> {
    let refs: Vec<CustomRef<'_>> = commands
        .iter()
        .map(|c| CustomRef {
            phrases: &c.phrases,
            mode: mode(c.mode),
            reply: &c.reply,
            enabled: c.enabled,
            priority: c.priority,
        })
        .collect();
    let (i, reply) = custom::find(&refs, message)?;
    Some((commands.get(i)?.name.clone(), reply))
}

fn slug(name: &str, taken: &[String]) -> String {
    let base: String = lib_quick::text::fold(name)
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|p| !p.is_empty())
        .collect::<Vec<_>>()
        .join("-");
    let base = if base.is_empty() {
        "polecenie".to_owned()
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
    AppError::invalid_field("commands", what)
}

/// Sprawdzenie jednego polecenia (nazwa, frazy, tryb, odpowiedź); poprawia białe znaki.
fn check(c: &mut CustomCommand) -> Result<(), AppError> {
    c.name = c.name.trim().to_owned();
    let label = if c.name.is_empty() {
        "bez nazwy".to_owned()
    } else {
        c.name.clone()
    };
    if c.name.is_empty() || c.name.chars().count() > MAX_NAME {
        return Err(bad(format!("„{label}”: nazwa od 1 do {MAX_NAME} znaków")));
    }
    c.phrases = c
        .phrases
        .iter()
        .map(|p| p.trim().to_owned())
        .filter(|p| !p.is_empty())
        .collect();
    if c.phrases.is_empty() || c.phrases.len() > MAX_PHRASES {
        return Err(bad(format!("„{label}”: od 1 do {MAX_PHRASES} fraz")));
    }
    for p in &c.phrases {
        if p.chars().count() > MAX_PHRASE {
            return Err(bad(format!(
                "„{label}”: fraza dłuższa niż {MAX_PHRASE} znaków"
            )));
        }
        if c.mode == QuickMatchMode::Regex {
            custom::compile(p).map_err(|e| bad(format!("„{label}”: złe wyrażenie „{p}” — {e}")))?;
        } else if !p.chars().any(char::is_alphanumeric) {
            return Err(bad(format!("„{label}”: fraza bez liter ani cyfr")));
        }
    }
    c.reply = c.reply.trim().to_owned();
    if c.reply.is_empty() || c.reply.chars().count() > MAX_REPLY {
        return Err(bad(format!(
            "„{label}”: odpowiedź od 1 do {MAX_REPLY} znaków"
        )));
    }
    c.priority = c.priority.clamp(-10, 10);
    Ok(())
}

/// Walidacja i normalizacja listy przed zapisem; wynik — wartość konfiguracji.
pub fn validate(commands: Vec<CustomCommand>) -> Result<serde_json::Value, AppError> {
    if commands.len() > MAX_COMMANDS {
        return Err(bad(format!("najwyżej {MAX_COMMANDS} poleceń")));
    }
    let mut out: Vec<CustomCommand> = Vec::with_capacity(commands.len());
    for mut c in commands {
        check(&mut c)?;
        let taken: Vec<String> = out.iter().map(|o| o.id.clone()).collect();
        if c.id.trim().is_empty() || taken.contains(&c.id) {
            c.id = slug(&c.name, &taken);
        }
        out.push(c);
    }
    let text = serde_json::to_string(&out).map_err(|e| AppError::internal(e.to_string()))?;
    Ok(serde_json::Value::String(text))
}

/// Widok strony.
pub fn view(value: Option<&serde_json::Value>, enabled: bool) -> QuickCommandsView {
    QuickCommandsView {
        commands: parse(value),
        builtins: lib_quick::CATALOG
            .iter()
            .map(|e| BuiltinCommand {
                title: e.title.to_owned(),
                examples: e.examples.iter().map(|x| (*x).to_owned()).collect(),
            })
            .collect(),
        enabled,
    }
}

/// Tester: kto odpowie (kolejność jak w rozmowie — własne, quiz, wbudowane, model).
pub fn test(value: Option<&serde_json::Value>, enabled: bool, message: &str) -> QuickTest {
    let result = |source, name: Option<String>, reply: Option<String>| QuickTest {
        source,
        name,
        reply,
    };
    if !enabled {
        return result(QuickSource::Disabled, None, None);
    }
    if let Some((name, reply)) = find(&parse(value), message) {
        return result(QuickSource::Custom, Some(name), Some(reply));
    }
    if let Some((_, first)) = lib_quick::quiz::start(message, &mut |n| n / 2) {
        return result(QuickSource::Quiz, None, Some(first));
    }
    let now = chrono::Local::now().naive_local();
    match lib_quick::answer(message, now, &mut |n| n / 2) {
        Some(r) => result(
            QuickSource::Builtin,
            Some(r.command.label().into()),
            Some(r.text),
        ),
        None => result(QuickSource::Model, None, None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn command(name: &str, phrases: &[&str], mode: QuickMatchMode, reply: &str) -> CustomCommand {
        CustomCommand {
            id: String::new(),
            name: name.into(),
            phrases: phrases.iter().map(|p| (*p).to_owned()).collect(),
            mode,
            reply: reply.into(),
            enabled: true,
            priority: 0,
        }
    }

    #[test]
    fn validate_store_and_test() {
        let value = validate(vec![
            command(
                "Dobranoc",
                &[" dobranoc "],
                QuickMatchMode::Exact,
                "Dobranoc!",
            ),
            command("Dobranoc", &["śpij"], QuickMatchMode::Exact, "Śpij dobrze."),
        ])
        .unwrap();
        let list = parse(Some(&value));
        assert_eq!(list[0].id, "dobranoc");
        assert_eq!(list[1].id, "dobranoc-2", "unikalne identyfikatory");
        assert_eq!(list[0].phrases, vec!["dobranoc".to_owned()]);
        let t = test(Some(&value), true, "Dobranoc!");
        assert_eq!(t.source, QuickSource::Custom);
        assert_eq!(t.reply.as_deref(), Some("Dobranoc!"));
        assert_eq!(test(Some(&value), true, "2+2").source, QuickSource::Builtin);
        assert_eq!(
            test(Some(&value), true, "tabliczka mnożenia").source,
            QuickSource::Quiz
        );
        assert_eq!(
            test(Some(&value), true, "Opowiedz bajkę").source,
            QuickSource::Model
        );
        assert_eq!(
            test(Some(&value), false, "2+2").source,
            QuickSource::Disabled
        );
        assert_eq!(
            view(Some(&value), true).builtins.len(),
            lib_quick::CATALOG.len()
        );

        for wrong in [
            command("", &["x"], QuickMatchMode::Exact, "y"),
            command("a", &[], QuickMatchMode::Exact, "y"),
            command("a", &["("], QuickMatchMode::Regex, "y"),
            command("a", &["!!!"], QuickMatchMode::Exact, "y"),
            command("a", &["x"], QuickMatchMode::Exact, " "),
        ] {
            assert!(validate(vec![wrong]).is_err());
        }
        let many = vec![command("a", &["x"], QuickMatchMode::Exact, "y"); 51];
        assert!(validate(many).is_err());
    }
}
