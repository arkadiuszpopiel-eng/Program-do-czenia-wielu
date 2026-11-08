//! Dekodowanie zdarzenia z XML-a `EvtRender(EvtRenderEventXml)` (przenośne, testowane na każdym
//! systemie): dostawca, identyfikator, poziom, czas `SystemTime` (ISO 8601 UTC → ms od epoki).

use platform_apps_contract::{EventLevel, EventRecord, MAX_EVENT_MESSAGE_CHARS, clip_chars};

/// Wartość atrybutu `name='…'` albo `name="…"` w pierwszym wystąpieniu znacznika `tag`.
fn attr(xml: &str, tag: &str, name: &str) -> Option<String> {
    let start = xml.find(&format!("<{tag}"))?;
    let rest = &xml[start..];
    let end = rest.find('>')?;
    let head = &rest[..end];
    for quote in ['\'', '"'] {
        let key = format!("{name}={quote}");
        if let Some(i) = head.find(&key) {
            let v = &head[i + key.len()..];
            return v.find(quote).map(|j| unescape(&v[..j]));
        }
    }
    None
}

/// Treść znacznika `<tag …>treść</tag>`.
fn text(xml: &str, tag: &str) -> Option<String> {
    let start = xml.find(&format!("<{tag}"))?;
    let rest = &xml[start..];
    let open_end = rest.find('>')?;
    if rest[..open_end].ends_with('/') {
        return Some(String::new());
    }
    let body = &rest[open_end + 1..];
    let close = body.find(&format!("</{tag}>"))?;
    Some(unescape(&body[..close]))
}

fn unescape(s: &str) -> String {
    s.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&amp;", "&")
}

/// Dni od 1970-01-01 dla daty kalendarzowej (algorytm „days from civil”).
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// `2024-05-01T10:00:00.1234567Z` → ms od epoki Unix (`None` dla innej postaci).
pub(crate) fn parse_system_time(s: &str) -> Option<u64> {
    let s = s.trim().strip_suffix('Z')?;
    let (date, time) = s.split_once('T')?;
    let mut d = date.split('-').map(|p| p.parse::<i64>().ok());
    let (y, mo, da) = (d.next()??, d.next()??, d.next()??);
    let (hms, frac) = time.split_once('.').unwrap_or((time, "0"));
    let mut t = hms.split(':').map(|p| p.parse::<i64>().ok());
    let (h, mi, se) = (t.next()??, t.next()??, t.next()??);
    if !(1..=12).contains(&mo) || !(1..=31).contains(&da) || h > 23 || mi > 59 || se > 60 {
        return None;
    }
    let ms_frac: i64 = format!("{frac:0<3}").get(..3)?.parse().ok()?;
    let secs = days_from_civil(y, mo, da) * 86_400 + h * 3_600 + mi * 60 + se;
    u64::try_from(secs * 1_000 + ms_frac).ok()
}

/// Zdarzenie z XML-a i (opcjonalnie) sformatowanego komunikatu.
pub(crate) fn record_from_xml(xml: &str, message: Option<String>) -> EventRecord {
    let provider = attr(xml, "Provider", "Name")
        .or_else(|| attr(xml, "Provider", "EventSourceName"))
        .unwrap_or_default();
    let event_id = text(xml, "EventID")
        .and_then(|t| t.trim().parse::<u32>().ok())
        .unwrap_or(0);
    let level = text(xml, "Level")
        .and_then(|t| t.trim().parse::<u8>().ok())
        .map_or(EventLevel::Information, EventLevel::from_value);
    let time_ms = attr(xml, "TimeCreated", "SystemTime")
        .and_then(|t| parse_system_time(&t))
        .unwrap_or(0);
    let message = message
        .filter(|m| !m.trim().is_empty())
        .or_else(|| text(xml, "Data"))
        .unwrap_or_default();
    EventRecord {
        time_ms,
        level,
        provider,
        event_id,
        message: clip_chars(message.trim(), MAX_EVENT_MESSAGE_CHARS),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const XML: &str = "<Event xmlns='http://schemas.microsoft.com/win/2004/08/events/event'><System>\
<Provider Name='Service Control Manager' Guid='{555908d1}' EventSourceName='Service Control Manager'/>\
<EventID Qualifiers='16384'>7036</EventID><Version>0</Version><Level>4</Level>\
<TimeCreated SystemTime='2024-05-01T10:00:00.1234567Z'/></System>\
<EventData><Data Name='param1'>Bufor &amp; wydruku</Data></EventData></Event>";

    #[test]
    fn decodes_system_fields() {
        let r = record_from_xml(XML, None);
        assert_eq!(r.provider, "Service Control Manager");
        assert_eq!(r.event_id, 7036);
        assert_eq!(r.level, EventLevel::Information);
        assert_eq!(r.time_ms, 1_714_557_600_123);
        assert_eq!(r.message, "Bufor & wydruku");
        let with = record_from_xml(XML, Some("Usługa weszła w stan zatrzymania.".into()));
        assert_eq!(with.message, "Usługa weszła w stan zatrzymania.");
        let empty = record_from_xml("<Event/>", None);
        assert_eq!((empty.event_id, empty.time_ms), (0, 0));
    }

    #[test]
    fn system_time_parsing() {
        assert_eq!(parse_system_time("1970-01-01T00:00:00Z"), Some(0));
        assert_eq!(
            parse_system_time("2000-03-01T00:00:01.5Z"),
            Some(951_868_801_500)
        );
        for bad in ["", "2024-13-01T00:00:00Z", "2024-01-01 00:00:00", "x"] {
            assert_eq!(parse_system_time(bad), None, "{bad}");
        }
    }
}
