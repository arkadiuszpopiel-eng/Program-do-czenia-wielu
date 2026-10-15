//! Własne polecenia użytkownika: fraza → stała odpowiedź (z podstawieniem `{reszta}` i nazwanych
//! grup wyrażenia). Tryby: cała wiadomość, początek, zawiera (pełne słowa), wyrażenie regularne.
//! Porównanie bez wielkości liter i polskich znaków, interpunkcja = odstęp; zwrot „hej Alfa”
//! na początku jest pomijany. Pierwszeństwo: wyższy priorytet, potem dłuższa fraza.

use regex::{Regex, RegexBuilder};

use crate::text::fold;

/// Najdłuższe wyrażenie regularne.
pub const MAX_PATTERN: usize = 300;
/// Najdłuższa wiadomość, do której pasują własne polecenia (dłuższa idzie do modelu).
pub const MAX_MESSAGE: usize = 2000;
/// Najdłuższa odpowiedź po podstawieniu zmiennych.
pub const MAX_REPLY: usize = 4000;

/// Tryb dopasowania frazy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// Cała wiadomość.
    Exact,
    /// Początek wiadomości; dalszy tekst to `{reszta}`.
    Starts,
    /// Fraza w wiadomości (pełne słowa).
    Contains,
    /// Wyrażenie regularne (bez wielkości liter); nazwane grupy to zmienne.
    Regex,
}

/// Polecenie do dopasowania (widok na dane wywołującego).
#[derive(Debug, Clone, Copy)]
pub struct CustomRef<'a> {
    /// Frazy.
    pub phrases: &'a [String],
    /// Tryb.
    pub mode: Mode,
    /// Szablon odpowiedzi.
    pub reply: &'a str,
    /// Włączone.
    pub enabled: bool,
    /// Priorytet (wyższy wygrywa).
    pub priority: i32,
}

/// Słowa z położeniem w oryginale: (początek, koniec, słowo po `fold`).
fn words(text: &str) -> Vec<(usize, usize, String)> {
    let mut out = Vec::new();
    let mut start = None;
    for (i, c) in text.char_indices() {
        match (c.is_alphanumeric(), start) {
            (true, None) => start = Some(i),
            (false, Some(s)) => {
                out.push((s, i, fold(&text[s..i])));
                start = None;
            }
            _ => {}
        }
    }
    if let Some(s) = start {
        out.push((s, text.len(), fold(&text[s..])));
    }
    out
}

/// Pomija zwrot do Alfy na początku („hej Alfa”, „Alfa”, „cześć Alfa”).
fn skip_address(ws: &[(usize, usize, String)]) -> usize {
    let first = ws.first().map(|w| w.2.as_str());
    let second = ws.get(1).map(|w| w.2.as_str());
    match (first, second) {
        (Some("hej" | "czesc"), Some("alfa")) => 2,
        (Some("alfa"), _) => 1,
        _ => 0,
    }
}

/// Kompilacja wyrażenia z limitami (bez wielkości liter, ≤ 300 znaków, ograniczony rozmiar).
pub fn compile(pattern: &str) -> Result<Regex, String> {
    if pattern.chars().count() > MAX_PATTERN {
        return Err(format!("wyrażenie dłuższe niż {MAX_PATTERN} znaków"));
    }
    RegexBuilder::new(pattern)
        .case_insensitive(true)
        .size_limit(1 << 20)
        .build()
        .map_err(|e| e.to_string())
}

/// Dopasowanie frazy: zmienne do podstawienia albo `None`.
pub fn match_phrase(mode: Mode, phrase: &str, message: &str) -> Option<Vec<(String, String)>> {
    if mode == Mode::Regex {
        let re = compile(phrase).ok()?;
        let caps = re.captures(message)?;
        let mut vars: Vec<(String, String)> = re
            .capture_names()
            .flatten()
            .filter_map(|n| Some((n.to_owned(), caps.name(n)?.as_str().trim().to_owned())))
            .collect();
        let end = caps.get(0)?.end();
        vars.push(("reszta".into(), message[end..].trim().to_owned()));
        return Some(vars);
    }
    let want: Vec<String> = words(phrase).into_iter().map(|w| w.2).collect();
    if want.is_empty() {
        return None;
    }
    let all = words(message);
    let ws = &all[skip_address(&all)..];
    let same = |slice: &[(usize, usize, String)]| slice.iter().map(|w| &w.2).eq(want.iter());
    let n = want.len();
    match mode {
        Mode::Exact => same(ws).then(Vec::new),
        Mode::Starts => {
            let head = ws.get(..n)?;
            if !same(head) {
                return None;
            }
            let end = head.last()?.1;
            let rest = message[end..].trim_start_matches(|c: char| !c.is_alphanumeric());
            Some(vec![("reszta".into(), rest.trim_end().to_owned())])
        }
        Mode::Contains => ws.windows(n).any(same).then(Vec::new),
        Mode::Regex => None,
    }
}

/// Podstawienie zmiennych `{nazwa}` (nieznane zostają bez zmian).
pub fn render(template: &str, vars: &[(String, String)]) -> String {
    let mut out = template.to_owned();
    for (name, value) in vars {
        out = out.replace(&format!("{{{name}}}"), value);
    }
    out
}

/// Pierwsze pasujące polecenie: indeks i gotowa odpowiedź.
pub fn find(commands: &[CustomRef<'_>], message: &str) -> Option<(usize, String)> {
    if message.chars().count() > MAX_MESSAGE {
        return None;
    }
    let mut best: Option<(i32, usize, usize, String)> = None;
    for (i, c) in commands.iter().enumerate().filter(|(_, c)| c.enabled) {
        for phrase in c.phrases {
            let Some(vars) = match_phrase(c.mode, phrase, message) else {
                continue;
            };
            let len = phrase.chars().count();
            let better = best
                .as_ref()
                .is_none_or(|(p, l, _, _)| (c.priority, len) > (*p, *l));
            if better {
                best = Some((c.priority, len, i, render(c.reply, &vars)));
            }
        }
    }
    let (_, _, i, reply) = best?;
    // Pusta odpowiedź (np. sama `{reszta}` bez dalszego tekstu) — wiadomość idzie dalej.
    let reply = reply.trim();
    if reply.is_empty() {
        return None;
    }
    let capped: String = reply.chars().take(MAX_REPLY).collect();
    Some((i, capped))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cmd<'a>(phrases: &'a [String], mode: Mode, reply: &'a str, priority: i32) -> CustomRef<'a> {
        CustomRef {
            phrases,
            mode,
            reply,
            enabled: true,
            priority,
        }
    }

    #[test]
    fn modes_variables_and_priority() {
        let night = vec!["dobranoc".to_owned()];
        let note = vec!["zanotuj".to_owned()];
        let office = vec!["godziny pracy".to_owned()];
        let city = vec![r"pogoda w (?P<miasto>\w+)".to_owned()];
        let cmds = [
            cmd(&night, Mode::Exact, "Dobranoc! Do jutra.", 0),
            cmd(&note, Mode::Starts, "Zapisuję: {reszta}", 0),
            cmd(&office, Mode::Contains, "Pracuję 8–16.", 0),
            cmd(&city, Mode::Regex, "Sprawdzam pogodę: {miasto}.", 0),
        ];
        let f = |m: &str| find(&cmds, m).map(|(_, r)| r);
        assert_eq!(
            f("Hej Alfa, dobranoc!").as_deref(),
            Some("Dobranoc! Do jutra.")
        );
        assert_eq!(f("dobranoc wszystkim"), None, "tryb dokładny");
        assert_eq!(
            f("Zanotuj: kupić mleko.").as_deref(),
            Some("Zapisuję: kupić mleko.")
        );
        assert_eq!(
            f("jakie masz GODZINY pracy?").as_deref(),
            Some("Pracuję 8–16.")
        );
        assert_eq!(f("godzinowy pracownik"), None, "pełne słowa");
        assert_eq!(
            f("Pogoda w Gdańsku").as_deref(),
            Some("Sprawdzam pogodę: Gdańsku.")
        );
        let hi = vec!["zanotuj pilne".to_owned()];
        let both = [
            cmd(&note, Mode::Starts, "zwykła", 0),
            cmd(&hi, Mode::Starts, "pilna", 0),
        ];
        assert_eq!(
            find(&both, "zanotuj pilne: x").map(|r| r.0),
            Some(1),
            "dłuższa fraza"
        );
        let ranked = [
            cmd(&note, Mode::Starts, "zwykła", 5),
            cmd(&hi, Mode::Starts, "pilna", 0),
        ];
        assert_eq!(
            find(&ranked, "zanotuj pilne: x").map(|r| r.0),
            Some(0),
            "priorytet"
        );
        assert!(compile("(").is_err() && compile(&"a".repeat(301)).is_err());
        let rest = vec!["powiedz".to_owned()];
        let only = [cmd(&rest, Mode::Starts, "{reszta}", 0)];
        assert_eq!(find(&only, "powiedz"), None, "pusta odpowiedź");
        let long = format!("powiedz {}", "x".repeat(MAX_MESSAGE));
        assert_eq!(find(&only, &long), None, "za długa wiadomość");
        let many = vec!["echo".to_owned()];
        let template = "{reszta}".repeat(50);
        let boom = [cmd(&many, Mode::Starts, &template, 0)];
        let out = find(&boom, &format!("echo {}", "y".repeat(500))).unwrap().1;
        assert_eq!(out.chars().count(), MAX_REPLY);
    }
}
