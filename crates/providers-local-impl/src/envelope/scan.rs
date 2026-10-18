//! Skanowanie tekstu koperty bez pełnego parsera: rozpoznanie pierwszego pola obiektu, granica
//! dekodowalnego prefiksu napisu JSON (sekwencje ucieczki pocięte w dowolnym miejscu), dekodowanie
//! fragmentu i nazwa narzędzia z uciętego wywołania. Wszystko bez paniki (tylko `get`, nigdy
//! indeksowanie z możliwym wyjściem poza granicę znaku).

/// Rozpoznany początek odpowiedzi.
pub(super) enum Prefix {
    /// Za mało tekstu, żeby rozstrzygnąć.
    Wait,
    /// To nie koperta.
    Plain,
    /// Pierwsze pole `response` — bajt, od którego zaczyna się treść napisu.
    Response(usize),
    /// Pierwsze pole `tool_call`.
    ToolCall,
}

/// Rozpoznaje początek koperty: `{`, klucz, `:` (białe znaki dowolne).
pub(super) fn classify(buf: &str) -> Prefix {
    let rest = buf.trim_start();
    let Some(after) = rest.strip_prefix('{').map(str::trim_start) else {
        return if rest.is_empty() {
            Prefix::Wait
        } else {
            Prefix::Plain
        };
    };
    let Some(key_and_rest) = after.strip_prefix('"') else {
        return if after.is_empty() {
            Prefix::Wait
        } else {
            Prefix::Plain
        };
    };
    let Some(end) = key_and_rest.find('"') else {
        return Prefix::Wait;
    };
    let key = key_and_rest.get(..end).unwrap_or_default();
    let after_key = key_and_rest.get(end + 1..).unwrap_or_default().trim_start();
    let Some(value) = after_key.strip_prefix(':').map(str::trim_start) else {
        return if after_key.is_empty() {
            Prefix::Wait
        } else {
            Prefix::Plain
        };
    };
    match key {
        "tool_call" => Prefix::ToolCall,
        "response" => match value.strip_prefix('"') {
            Some(content) => Prefix::Response(buf.len() - content.len()),
            None if value.is_empty() => Prefix::Wait,
            None => Prefix::Plain,
        },
        _ => Prefix::Plain,
    }
}

/// Skanuje surową treść napisu JSON (bez otwierającego `"`): długość prefiksu, który da się już
/// zdekodować (całe sekwencje ucieczki, granica znaku), i czy napis się zamknął (wtedy długość
/// = pozycja zamykającego `"`).
pub(super) fn string_scan(raw: &str) -> (usize, bool) {
    let bytes = raw.as_bytes();
    let (mut at, mut safe) = (0usize, 0usize);
    while let Some(&b) = bytes.get(at) {
        match b {
            b'"' => return (at, true),
            b'\\' => {
                let need = match bytes.get(at + 1) {
                    None => return (safe, false),
                    Some(b'u') => {
                        let Some(hex) = raw.get(at + 2..at + 6) else {
                            return (safe, false);
                        };
                        let code = u32::from_str_radix(hex, 16).unwrap_or(0);
                        // Wysoki surogat: dekodujemy dopiero z parą `\uDCxx`.
                        if (0xD800..0xDC00).contains(&code) {
                            12
                        } else {
                            6
                        }
                    }
                    Some(_) => 2,
                };
                if at + need > bytes.len() {
                    return (safe, false);
                }
                at += need;
            }
            _ => at += 1,
        }
        if raw.is_char_boundary(at) {
            safe = at;
        }
    }
    (safe, false)
}

/// Dekoduje kompletny fragment treści napisu JSON; surowe znaki sterujące (niepoprawne w JSON)
/// są dopuszczane, a fragment nie do zdekodowania wraca bez zmian.
pub(super) fn decode(raw: &str) -> String {
    if !raw.contains('\\') && !raw.contains(char::is_control) {
        return raw.to_owned();
    }
    let lenient = raw
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t");
    serde_json::from_str::<String>(&format!("\"{lenient}\"")).unwrap_or_else(|_| raw.to_owned())
}

/// Nazwa narzędzia z uciętego JSON-a (`"name": "…"`), żeby błąd trafił do właściwego wywołania.
pub(super) fn name_hint(buf: &str) -> Option<String> {
    let after = buf
        .split_once("\"name\"")?
        .1
        .trim_start()
        .strip_prefix(':')?;
    let value = after.trim_start().strip_prefix('"')?;
    let (name, _) = value.split_once('"')?;
    Some(name.to_owned())
}
