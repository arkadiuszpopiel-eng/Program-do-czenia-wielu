//! Zapis poziomów autonomii po stronie aplikacji (konfiguracja rdzenia, nie Broker): obniżenia
//! sesji (`identyfikator-sesji:poziom`) i podniesienia czekające na zgodę w oknie Brokera
//! (`zakres:poziom:chwila`, `*` — globalny). Konfiguracja nie przyjmuje obiektów jako wartości —
//! stąd listy napisów. Wydzielone z `app-core` (strażnik autonomii; limit 8000 linii crate'a).

use std::collections::BTreeMap;

use app_api::dto::AutonomyLevel;

/// Podniesienia czekające na zgodę: zakres → (poziom, chwila prośby w ms).
pub type PendingRaises = BTreeMap<String, (AutonomyLevel, i64)>;

fn level_text(level: AutonomyLevel) -> Option<String> {
    serde_json::to_value(level)
        .ok()?
        .as_str()
        .map(str::to_owned)
}

fn level_of(text: &str) -> Option<AutonomyLevel> {
    serde_json::from_value(serde_json::Value::String(text.into())).ok()
}

/// Obniżenia sesji z wpisów `identyfikator-sesji:poziom` (wpisy nieczytelne są pomijane).
pub fn parse_session_levels(entries: Vec<String>) -> BTreeMap<String, AutonomyLevel> {
    entries
        .into_iter()
        .filter_map(|entry| {
            let (id, level) = entry.rsplit_once(':')?;
            Some((id.to_owned(), level_of(level)?))
        })
        .collect()
}

/// Wpisy `identyfikator-sesji:poziom`.
pub fn format_session_levels(levels: &BTreeMap<String, AutonomyLevel>) -> Vec<String> {
    levels
        .iter()
        .map(|(id, level)| format!("{id}:{}", level_text(*level).unwrap_or_default()))
        .collect()
}

/// Oczekujące podniesienia z wpisów `zakres:poziom:chwila`, bez starszych niż `ttl_ms` od `now_ms`.
pub fn parse_raises(entries: Vec<String>, now_ms: i64, ttl_ms: i64) -> PendingRaises {
    entries
        .into_iter()
        .filter_map(|entry| {
            let mut parts = entry.rsplitn(3, ':');
            let at: i64 = parts.next()?.parse().ok()?;
            let level = level_of(parts.next()?)?;
            let scope = parts.next()?;
            (now_ms.saturating_sub(at) <= ttl_ms).then(|| (scope.to_owned(), (level, at)))
        })
        .collect()
}

/// Wpisy `zakres:poziom:chwila`.
pub fn format_raises(raises: &PendingRaises) -> Vec<String> {
    raises
        .iter()
        .filter_map(|(scope, (level, at))| Some(format!("{scope}:{}:{at}", level_text(*level)?)))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entries_round_trip_and_bad_or_stale_ones_are_skipped() {
        let sessions: BTreeMap<String, AutonomyLevel> = [
            ("s-1".to_owned(), AutonomyLevel::L0),
            ("a:b".to_owned(), AutonomyLevel::L2),
        ]
        .into();
        let text = format_session_levels(&sessions);
        assert_eq!(text, vec!["a:b:L2".to_owned(), "s-1:L0".to_owned()]);
        assert_eq!(parse_session_levels(text), sessions);
        assert!(parse_session_levels(vec!["bez-poziomu".into(), "s:L9".into()]).is_empty());

        let raises: PendingRaises = [("*".to_owned(), (AutonomyLevel::L4, 1_000))].into();
        let text = format_raises(&raises);
        assert_eq!(text, vec!["*:L4:1000".to_owned()]);
        assert_eq!(parse_raises(text.clone(), 2_000, 5_000), raises);
        // Przeterminowane (karta Brokera wygasła) i nieczytelne — pomijane.
        assert!(parse_raises(text, 10_000, 5_000).is_empty());
        assert!(parse_raises(vec!["*:L4".into(), "*:X:1".into()], 0, 5_000).is_empty());
    }
}
