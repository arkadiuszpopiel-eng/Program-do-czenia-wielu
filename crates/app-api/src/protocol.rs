//! Handler protokołu `alfa://` (shell-integration): parsowanie URI z listą dozwolonych akcji.
//! Dane z protokołu są niezaufanym wejściem — nieznana akcja, zbyt długi URI albo znaki
//! sterujące są odrzucane; tekst nigdy nie jest wysyłany automatycznie (tylko wstawiany do UI).

/// Maksymalna długość URI.
pub const MAX_URI_LEN: usize = 2048;

/// Akcja z URI `alfa://…`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProtocolAction {
    /// `alfa://open` — pokaż okno główne.
    Open,
    /// `alfa://session/<id>` — otwórz sesję.
    OpenSession(String),
    /// `alfa://new?text=…` — nowa rozmowa (tekst trafia do szkicu, nie jest wysyłany).
    NewChat(Option<String>),
    /// `alfa://quick` — Szybkie pytanie.
    QuickAsk,
}

fn decode(s: &str) -> Option<String> {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'%' => {
                let hex = s.get(i + 1..i + 3)?;
                out.push(u8::from_str_radix(hex, 16).ok()?);
                i += 3;
            }
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            b => {
                out.push(b);
                i += 1;
            }
        }
    }
    String::from_utf8(out).ok()
}

/// Parsuje `alfa://…`; `None` = odrzucone (spoza listy dozwolonych).
pub fn parse_uri(uri: &str) -> Option<ProtocolAction> {
    if uri.len() > MAX_URI_LEN {
        return None;
    }
    let rest = uri.strip_prefix("alfa://")?;
    let (path, query) = rest.split_once('?').unwrap_or((rest, ""));
    let path = path.trim_end_matches('/');
    let action = match path.split_once('/') {
        None if path == "open" || path.is_empty() => ProtocolAction::Open,
        None if path == "quick" => ProtocolAction::QuickAsk,
        None if path == "new" => {
            let text = match query.split('&').find_map(|kv| kv.strip_prefix("text=")) {
                Some(raw) => Some(decode(raw)?).filter(|t| !t.trim().is_empty()),
                None => None,
            };
            ProtocolAction::NewChat(text)
        }
        Some(("session", id)) => {
            let id = decode(id)?;
            let ok = !id.is_empty()
                && id.len() <= 128
                && id
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
            if !ok {
                return None;
            }
            ProtocolAction::OpenSession(id)
        }
        _ => return None,
    };
    let control = |a: &ProtocolAction| match a {
        ProtocolAction::NewChat(Some(t)) => t.chars().any(|c| c.is_control() && c != '\n'),
        _ => false,
    };
    (!control(&action)).then_some(action)
}

/// Pierwszy argument wiersza poleceń będący URI `alfa://` (druga instancja → pierwsza).
pub fn from_args<I: IntoIterator<Item = String>>(args: I) -> Option<ProtocolAction> {
    args.into_iter().find_map(|a| parse_uri(a.trim()))
}
