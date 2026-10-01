//! Karty zgodności tras mostów (Ustawienia → Modele i dostawcy → Mosty): stan trasy
//! z `compliance` (zielona/szara/zabroniona, nieświeży wpis), wersja wykryta vs przypięta,
//! źródła regulaminu, data weryfikacji, wyłącznik i zgoda na harmonogram.

use accounts_hub_contract::CliBridge;
use agent_backends_contract::BridgeKind;
use app_api::dto::{BridgeCard, BridgeSource, ComplianceStatus};
use compliance_contract::{RegistryRoute, RouteMode, RouteStatus, RouteView};

/// Most obsługiwany przez Alfę dla trasy rejestru.
pub fn bridge_of_route(route: &str) -> Option<BridgeKind> {
    BridgeKind::ALL
        .into_iter()
        .find(|b| b.route_id_str() == route)
}

/// Nazwa mostu w DTO i kluczach konfiguracji.
pub fn bridge_key(kind: BridgeKind) -> &'static str {
    match kind {
        BridgeKind::ClaudeCode => "claude_code",
        BridgeKind::Codex => "codex",
    }
}

/// Most z nazwy DTO.
pub fn bridge_from_key(key: &str) -> Option<BridgeKind> {
    BridgeKind::ALL.into_iter().find(|b| bridge_key(*b) == key)
}

/// Polecenie logowania, które użytkownik wpisuje sam (Alfa go nie wykonuje).
pub fn login_command(kind: BridgeKind) -> &'static str {
    match kind {
        BridgeKind::ClaudeCode => "claude /login",
        BridgeKind::Codex => "codex login",
    }
}

fn status(s: RouteStatus) -> ComplianceStatus {
    match s {
        RouteStatus::Green => ComplianceStatus::Green,
        RouteStatus::Grey => ComplianceStatus::Gray,
        RouteStatus::Forbidden => ComplianceStatus::Forbidden,
    }
}

fn mode(m: RouteMode) -> &'static str {
    match m {
        RouteMode::CliHeadless => "cli-p",
        RouteMode::AgentSdk => "sdk",
        RouteMode::Api => "api",
    }
}

/// Nazwa trasy dla UI.
pub fn route_name(id: &str) -> String {
    match id {
        "claude-code-cli" => "Claude Code (CLI)".into(),
        "claude-agent-sdk" => "Claude Agent SDK".into(),
        "codex-cli" => "Codex CLI".into(),
        "grok-build-subscription" => "Grok Build (subskrypcja)".into(),
        "grok-build-api" => "Grok Build (klucz API)".into(),
        "kimi-code-cli" => "Kimi Code CLI".into(),
        "agy-antigravity" => "Antigravity CLI (agy)".into(),
        "qwen-coding-plan" => "Qwen Coding Plan".into(),
        "glm-zcode-plan" => "GLM / ZCode Plan".into(),
        other => other.replace('-', " "),
    }
}

/// Dane mostu spoza rejestru (wykrycie, konfiguracja użytkownika).
pub struct CardInput<'a> {
    /// Widok trasy.
    pub view: &'a RouteView,
    /// Wpis rejestru.
    pub route: Option<&'a RegistryRoute>,
    /// Wykryte CLI.
    pub detected: Option<&'a CliBridge>,
    /// Wersje przypięte w konfiguracji.
    pub pinned: Vec<String>,
    /// Zgoda na harmonogram (na dobę).
    pub schedule: u32,
}

/// Karta zgodności.
pub fn card(input: CardInput<'_>) -> BridgeCard {
    let CardInput {
        view,
        route,
        detected,
        pinned,
        schedule,
    } = input;
    let bridge = bridge_of_route(view.id.as_str());
    let registry_pin = route.and_then(|r| r.cli_pinned_version.clone());
    let version = detected.and_then(|d| d.version.clone());
    let version_ok = version
        .as_ref()
        .is_some_and(|v| pinned.contains(v) && registry_pin.as_ref().is_none_or(|r| r == v));
    let forbidden = view.effective.status == RouteStatus::Forbidden;
    BridgeCard {
        route_id: view.id.to_string(),
        bridge: bridge.map(|b| bridge_key(b).to_owned()),
        name: route_name(view.id.as_str()),
        provider: view.provider.clone(),
        mode: mode(view.mode).into(),
        program: bridge.map(|b| b.program().to_owned()),
        detected: detected.is_some(),
        path: detected.map(|d| d.path.to_string_lossy().into_owned()),
        version,
        pinned,
        registry_pin,
        version_ok,
        status: status(view.effective.status),
        stale: view.effective.stale,
        verified_at: view.verified_at.map(|d| d.to_string()),
        sources: route.map_or_else(Vec::new, |r| {
            r.sources
                .iter()
                .filter(|s| s.url.starts_with("https://"))
                .map(|s| BridgeSource {
                    url: s.url.clone(),
                    quote: s.quote.chars().take(300).collect(),
                })
                .collect()
        }),
        allowed: route.map_or_else(Vec::new, |r| r.allowed.clone()),
        forbidden: route.map_or_else(Vec::new, |r| r.forbidden.clone()),
        enabled: view.enabled && !forbidden,
        can_enable: !forbidden && bridge.is_some(),
        schedule_per_day: schedule,
        login_command: bridge.map(|b| login_command(b).to_owned()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn route_and_bridge_names() {
        assert_eq!(
            bridge_of_route("claude-code-cli"),
            Some(BridgeKind::ClaudeCode)
        );
        assert_eq!(bridge_of_route("kimi-code-cli"), None);
        for b in BridgeKind::ALL {
            assert_eq!(bridge_from_key(bridge_key(b)), Some(b));
        }
        assert_eq!(login_command(BridgeKind::Codex), "codex login");
    }
}
