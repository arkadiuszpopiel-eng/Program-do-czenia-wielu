//! Filtr „zmyślonych wywołań narzędzi”. Mały model lokalny, który nie dostał żadnych narzędzi,
//! potrafi dopisać do odpowiedzi tekst w kształcie wywołania — `{"name": "get_status",
//! "arguments": {…}}` albo blok `<tool_call>…</tool_call>` (zgłoszenie właściciela, 2026-10-09:
//! „po co te informacje są widoczne po wypowiedzi”). To nie jest treść dla człowieka i nic jej
//! nie wykona, więc wycinamy ją ze strumienia. Filtr działa tylko dla żądań bez narzędzi; zwykły
//! JSON w odpowiedzi (np. przykład pliku konfiguracji) przechodzi bez zmian.

use lib_openai_compat::StreamDecoder;
use lib_openai_compat::sse::SseEvent;
use providers_contract::ProviderEvent;

/// Najdłuższy fragment wstrzymywany w oczekiwaniu na domknięcie obiektu (znaki).
const MAX_HELD: usize = 4_096;
const OPEN_TAG: &str = "<tool_call>";
const CLOSE_TAG: &str = "</tool_call>";

/// Czy tekst jest obiektem JSON w kształcie wywołania narzędzia.
fn is_tool_call(text: &str) -> bool {
    let Ok(serde_json::Value::Object(map)) = serde_json::from_str::<serde_json::Value>(text) else {
        return false;
    };
    let named = map.get("name").is_some_and(serde_json::Value::is_string);
    let args = ["arguments", "parameters"].iter().any(|key| {
        map.get(*key)
            .is_some_and(|v| v.is_object() || v.is_string())
    });
    named && args && map.len() <= 3
}

/// Koniec pierwszego kompletnego obiektu JSON zaczynającego się na początku `text` (indeks za `}`).
fn object_end(text: &str) -> Option<usize> {
    let (mut depth, mut in_string, mut escaped) = (0usize, false, false);
    for (at, c) in text.char_indices() {
        if in_string {
            match c {
                _ if escaped => escaped = false,
                '\\' => escaped = true,
                '"' => in_string = false,
                _ => {}
            }
            continue;
        }
        match c {
            '"' => in_string = true,
            '{' => depth += 1,
            '}' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return Some(at + 1);
                }
            }
            _ => {}
        }
    }
    None
}

/// Filtr strumienia tekstu: oddaje to, co na pewno jest treścią, wstrzymuje możliwe wywołanie.
#[derive(Debug, Default)]
pub struct LeakFilter {
    held: String,
}

impl LeakFilter {
    /// Dokłada fragment; zwraca tekst gotowy do pokazania.
    pub fn push(&mut self, text: &str) -> String {
        self.held.push_str(text);
        self.drain(false)
    }

    /// Koniec odpowiedzi: zwraca resztę (urwane wywołanie narzędzia przepada).
    pub fn finish(&mut self) -> String {
        self.drain(true)
    }

    fn drain(&mut self, end: bool) -> String {
        let mut out = String::new();
        loop {
            let brace = self.held.find('{');
            let tag = self.held.find(OPEN_TAG);
            let start = match (brace, tag) {
                (Some(b), Some(t)) => b.min(t),
                (Some(at), None) | (None, Some(at)) => at,
                (None, None) => {
                    // Początek znacznika może być urwany na końcu fragmentu (`<tool_`).
                    let keep = if end { 0 } else { partial_tag(&self.held) };
                    let cut = self.held.len() - keep;
                    out.push_str(&self.held[..cut]);
                    self.held.drain(..cut);
                    return out;
                }
            };
            out.push_str(&self.held[..start]);
            self.held.drain(..start);
            if self.held.starts_with(OPEN_TAG) {
                match self.held.find(CLOSE_TAG) {
                    Some(close) => {
                        self.held.drain(..close + CLOSE_TAG.len());
                        continue;
                    }
                    None if end || self.held.len() > MAX_HELD => {
                        // Niedomknięty blok wywołania: do końca odpowiedzi to nadal nie treść.
                        self.held.clear();
                        return out;
                    }
                    None => return out,
                }
            }
            match object_end(&self.held) {
                Some(close) => {
                    let object: String = self.held.drain(..close).collect();
                    if !is_tool_call(&object) {
                        out.push_str(&object);
                    }
                }
                None if end => {
                    // Urwany obiekt: wywołanie poznajemy po początku, resztę oddajemy.
                    let compact: String = self.held.split_whitespace().collect();
                    if !compact.starts_with("{\"name\":") {
                        out.push_str(&self.held);
                    }
                    self.held.clear();
                    return out;
                }
                None if self.held.len() > MAX_HELD => {
                    out.push_str(&self.held);
                    self.held.clear();
                    return out;
                }
                None => return out,
            }
        }
    }
}

/// Długość końcówki, która może być początkiem `<tool_call>`.
fn partial_tag(text: &str) -> usize {
    (1..OPEN_TAG.len())
        .rev()
        .find(|n| text.ends_with(&OPEN_TAG[..*n]))
        .unwrap_or(0)
}

/// Dekoder z filtrem zmyślonych wywołań (włączonym tylko dla żądań bez narzędzi).
pub struct LeakGuard<D> {
    inner: D,
    filter: Option<LeakFilter>,
    /// Indeks bloku tekstu, z którego pochodzi wstrzymany fragment.
    index: u32,
}

impl<D> LeakGuard<D> {
    /// Opakowuje dekoder; `enabled` = żądanie nie niosło narzędzi.
    pub fn new(inner: D, enabled: bool) -> Self {
        Self {
            inner,
            filter: enabled.then(LeakFilter::default),
            index: 0,
        }
    }

    fn filter(&mut self, events: Vec<ProviderEvent>) -> Vec<ProviderEvent> {
        let Some(filter) = self.filter.as_mut() else {
            return events;
        };
        let mut out = Vec::with_capacity(events.len());
        for event in events {
            match event {
                ProviderEvent::TextDelta { index, text } => {
                    self.index = index;
                    let shown = filter.push(&text);
                    if !shown.is_empty() {
                        out.push(ProviderEvent::TextDelta { index, text: shown });
                    }
                }
                terminal if terminal.is_terminal() => {
                    let rest = filter.finish();
                    if !rest.is_empty() {
                        out.push(ProviderEvent::TextDelta {
                            index: self.index,
                            text: rest,
                        });
                    }
                    out.push(terminal);
                }
                other => out.push(other),
            }
        }
        out
    }
}

impl<D: StreamDecoder> StreamDecoder for LeakGuard<D> {
    fn on_event(&mut self, event: SseEvent) -> Vec<ProviderEvent> {
        let events = self.inner.on_event(event);
        self.filter(events)
    }

    fn on_eof(&mut self) -> Vec<ProviderEvent> {
        let events = self.inner.on_eof();
        self.filter(events)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(chunks: &[&str]) -> String {
        let mut filter = LeakFilter::default();
        let mut out: String = chunks.iter().map(|c| filter.push(c)).collect();
        out.push_str(&filter.finish());
        out
    }

    /// Dokładnie odpowiedź z laptopa właściciela, w całości i pocięta w każdym miejscu.
    #[test]
    fn invented_tool_call_is_removed_however_it_is_chunked() {
        let reply = "Wszystko dobrze. Dziękuję. {\"name\": \"get_status\", \"arguments\": {\"service\": \"Scheduler\"}}";
        assert_eq!(run(&[reply]), "Wszystko dobrze. Dziękuję. ");
        let chars: Vec<char> = reply.chars().collect();
        for cut in 1..chars.len() {
            let (a, b): (String, String) =
                (chars[..cut].iter().collect(), chars[cut..].iter().collect());
            assert_eq!(
                run(&[&a, &b]),
                "Wszystko dobrze. Dziękuję. ",
                "cięcie {cut}"
            );
        }
        let one_by_one: Vec<String> = chars.iter().map(char::to_string).collect();
        let refs: Vec<&str> = one_by_one.iter().map(String::as_str).collect();
        assert_eq!(run(&refs), "Wszystko dobrze. Dziękuję. ");
    }

    #[test]
    fn tagged_and_truncated_calls_are_removed_too() {
        assert_eq!(
            run(&[
                "Sprawdzam. <tool_",
                "call>{\"name\":\"x\",\"arguments\":{}}</tool_call> Gotowe."
            ]),
            "Sprawdzam.  Gotowe."
        );
        assert_eq!(run(&["Już. <tool_call>{\"name\": \"x\""]), "Już. ");
        assert_eq!(run(&["Już. {\"name\": \"get_status\", \"argu"]), "Już. ");
        assert_eq!(
            run(&[
                "A {\"name\":\"a\",\"parameters\":{}} B {\"name\":\"b\",\"arguments\":\"{}\"} C"
            ]),
            "A  B  C"
        );
    }

    #[test]
    fn ordinary_text_and_json_pass_through() {
        for text in [
            "Zwykła odpowiedź bez klamer.",
            "Plik: {\"port\": 8080, \"host\": \"localhost\"} — gotowe.",
            "Osoba: {\"name\": \"Ala\", \"wiek\": 30}",
            "Funkcja w Rust: fn main() { println!(\"{}\", 1); }",
            "Mniejsze niż: a < b oraz <tool> i <tool_callx>",
            "Urwany JSON: {\"port\": 80",
            "Tekst w klamrach \"}\" i {\"a\": \"}\"}",
        ] {
            assert_eq!(run(&[text]), text, "{text}");
            let mid = (0..=text.len() / 2)
                .rev()
                .find(|at| text.is_char_boundary(*at))
                .unwrap_or(0);
            let (a, b) = text.split_at(mid);
            assert_eq!(run(&[a, b]), text, "{text} (pocięte)");
        }
    }

    #[test]
    fn very_long_unclosed_object_is_released() {
        let long = format!("{{\"dane\": \"{}", "x".repeat(MAX_HELD + 10));
        let mut filter = LeakFilter::default();
        assert_eq!(filter.push(&long), long);
    }
}
