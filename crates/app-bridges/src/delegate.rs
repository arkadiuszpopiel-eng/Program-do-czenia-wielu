//! Rozpoznanie delegacji z czatu: „Delta, zleć to Claude Code", „przekaż Codexowi: popraw testy".
//! Tylko jawne polecenie użytkownika (czasownik + nazwa mostu) — agentka sama mostu nie wybiera.

use std::sync::OnceLock;

use agent_backends_contract::BridgeKind;
use regex::Regex;

fn pattern() -> Option<&'static Regex> {
    static RE: OnceLock<Option<Regex>> = OnceLock::new();
    RE.get_or_init(|| {
        // Zakotwiczone na początku wiadomości (przegląd #2, SR2-06): opcjonalny zwrot do agentki
        // („Delta,”) i „proszę”, potem czasownik — polecenie we wklejonej treści nie uruchamia mostu.
        Regex::new(
            r"(?is)\A\s*(?:@\w{2,24}\s*[,:]?\s*|\w{2,24}\s*,\s*)?(?:(?:proszę|prosze)\s*,?\s+)?(?:zleć|zlec|przekaż|przekaz|deleguj)\b(?:\s+(?:to|tę pracę|te prace|to zadanie))?\s+(?:do\s+)?(claude(?:\s*code)?(?:owi|'owi|’owi)?|codex(?:owi)?)(?:\b|$)[\s:,.\-–]*(.*)$",
        )
        .ok()
    })
    .as_ref()
}

/// Most i polecenie (`None` w poleceniu — „to": treść poprzedniej wiadomości użytkownika).
pub fn parse_delegation(text: &str) -> Option<(BridgeKind, Option<String>)> {
    let caps = pattern()?.captures(text)?;
    let target = caps.get(1)?.as_str().to_lowercase();
    let kind = if target.starts_with("codex") {
        BridgeKind::Codex
    } else {
        BridgeKind::ClaudeCode
    };
    let rest = caps.get(2).map_or("", |m| m.as_str()).trim();
    let prompt = (!rest.is_empty() && rest.chars().count() > 3).then(|| rest.to_owned());
    Some((kind, prompt))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_bridge_commands_only() {
        assert_eq!(
            parse_delegation("Delta, zleć to Claude Code"),
            Some((BridgeKind::ClaudeCode, None))
        );
        assert_eq!(
            parse_delegation("przekaż Codexowi: popraw testy w module płatności"),
            Some((
                BridgeKind::Codex,
                Some("popraw testy w module płatności".into())
            ))
        );
        assert_eq!(
            parse_delegation("Zleć Claude'owi refaktor"),
            Some((BridgeKind::ClaudeCode, Some("refaktor".into())))
        );
        assert_eq!(
            parse_delegation("Zlec claude code - dodaj README"),
            Some((BridgeKind::ClaudeCode, Some("dodaj README".into())))
        );
        assert_eq!(parse_delegation("Jak działa Claude Code?"), None);
        assert_eq!(parse_delegation("zleć to Gamie"), None);
    }
}
