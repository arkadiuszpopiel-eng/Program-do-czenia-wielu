//! Stan logowania mostu CLI bez czytania danych logowania (AGENTS.md): Alfa uruchamia oficjalne
//! polecenie stanu narzędzia — `claude auth status`, `codex login status` — i zostawia z wyniku
//! tylko „zalogowano / nie”, sposób logowania i rodzaj planu. Adres e-mail, identyfikatory
//! organizacji i ścieżki konfiguracji są odrzucane od razu. Antigravity (`agy`) nie ma osobnego
//! polecenia stanu — jego logowanie widać w jego oknie.

use accounts_hub_contract::CliOutput;
use agent_backends_contract::BridgeKind;
use app_api::dto::{BridgeAccount, BridgeAccountState};

/// Argumenty polecenia stanu logowania (`None` — narzędzie go nie ma).
pub fn status_args(kind: BridgeKind) -> Option<&'static [&'static str]> {
    match kind {
        BridgeKind::ClaudeCode => Some(&["auth", "status", "--json"]),
        BridgeKind::Codex => Some(&["login", "status"]),
        BridgeKind::Antigravity => None,
    }
}

fn account(
    state: BridgeAccountState,
    plan: Option<String>,
    method: Option<String>,
    detail: &str,
) -> BridgeAccount {
    BridgeAccount {
        state,
        plan,
        method,
        detail: detail.to_owned(),
        checked_at: app_api::dto::iso(chrono::Utc::now()),
    }
}

/// Nazwa planu Claude po ludzku (`max` → „Max”).
fn claude_plan(raw: &str) -> String {
    match raw.to_ascii_lowercase().as_str() {
        "max" => "Max".to_owned(),
        "pro" => "Pro".to_owned(),
        "team" => "Team".to_owned(),
        "enterprise" => "Enterprise".to_owned(),
        "free" => "bezpłatny".to_owned(),
        other => other.to_owned(),
    }
}

/// Wynik `claude auth status --json`: `loggedIn`, `authMethod`, `subscriptionType`.
pub fn parse_claude(out: &CliOutput) -> BridgeAccount {
    let json = out
        .text
        .find('{')
        .and_then(|at| serde_json::from_str::<serde_json::Value>(&out.text[at..]).ok());
    let Some(json) = json else {
        return account(
            BridgeAccountState::Error,
            None,
            None,
            "Claude Code nie podał stanu logowania (nieoczekiwana odpowiedź).",
        );
    };
    let logged = json["loggedIn"].as_bool().unwrap_or(false);
    let method = json["authMethod"].as_str().map(|m| match m {
        "claude.ai" => "konto claude.ai".to_owned(),
        "api_key" | "apiKey" => "klucz API".to_owned(),
        other => other.to_owned(),
    });
    let plan = json["subscriptionType"].as_str().map(claude_plan);
    if logged {
        let detail = match &plan {
            Some(p) => format!("Zalogowano — plan {p}. Alfa może zlecać zadania Claude Code."),
            None => "Zalogowano. Alfa może zlecać zadania Claude Code.".to_owned(),
        };
        account(BridgeAccountState::LoggedIn, plan, method, &detail)
    } else {
        account(
            BridgeAccountState::LoggedOut,
            None,
            None,
            "Nie zalogowano — kliknij „Zaloguj” i zaloguj się w oknie Claude Code.",
        )
    }
}

/// Wynik `codex login status`: „Logged in using ChatGPT” / „… an API key” / „Not logged in”.
pub fn parse_codex(out: &CliOutput) -> BridgeAccount {
    let text = out.text.to_ascii_lowercase();
    if text.contains("not logged in") || (!out.success && !text.contains("logged in")) {
        return account(
            BridgeAccountState::LoggedOut,
            None,
            None,
            "Nie zalogowano — kliknij „Zaloguj” i zaloguj się w oknie Codex.",
        );
    }
    if text.contains("logged in") {
        let method = if text.contains("chatgpt") {
            Some("konto ChatGPT".to_owned())
        } else if text.contains("api key") {
            Some("klucz API".to_owned())
        } else {
            None
        };
        let detail = match &method {
            Some(m) => format!("Zalogowano ({m}). Alfa może zlecać zadania Codex."),
            None => "Zalogowano. Alfa może zlecać zadania Codex.".to_owned(),
        };
        return account(BridgeAccountState::LoggedIn, None, method, &detail);
    }
    account(
        BridgeAccountState::Error,
        None,
        None,
        "Codex nie podał stanu logowania (nieoczekiwana odpowiedź).",
    )
}

/// Stan konta mostu z wyniku polecenia stanu (`None` — polecenie się nie uruchomiło).
pub fn check(kind: BridgeKind, out: Option<&CliOutput>) -> BridgeAccount {
    match (kind, out) {
        (BridgeKind::Antigravity, _) => account(
            BridgeAccountState::Unknown,
            None,
            None,
            "Antigravity nie ma polecenia stanu logowania — zaloguj się i sprawdź konto w oknie agy (Zaloguj). Zadań Alfa mu nie zleca, dopóki nie włączysz mostu.",
        ),
        (_, None) => account(
            BridgeAccountState::Error,
            None,
            None,
            "Nie udało się uruchomić polecenia stanu logowania (brak odpowiedzi w limicie czasu).",
        ),
        (BridgeKind::ClaudeCode, Some(out)) => parse_claude(out),
        (BridgeKind::Codex, Some(out)) => parse_codex(out),
    }
}

/// Konto, gdy narzędzia nie ma na komputerze.
pub fn not_installed(kind: BridgeKind) -> BridgeAccount {
    let name = match kind {
        BridgeKind::ClaudeCode => "Claude Code",
        BridgeKind::Codex => "Codex",
        BridgeKind::Antigravity => "Antigravity CLI (agy)",
    };
    account(
        BridgeAccountState::NotInstalled,
        None,
        None,
        &format!("{name} nie jest zainstalowany na tym komputerze."),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn out(success: bool, text: &str) -> CliOutput {
        CliOutput {
            success,
            text: text.to_owned(),
        }
    }

    #[test]
    fn claude_status_keeps_only_state_method_and_plan() {
        let json = r#"{"loggedIn": true, "authMethod": "claude.ai", "email": "ala@example.com",
            "orgId": "577ee1b4", "subscriptionType": "max"}"#;
        let a = parse_claude(&out(true, json));
        assert_eq!(a.state, BridgeAccountState::LoggedIn);
        assert_eq!(a.plan.as_deref(), Some("Max"));
        assert_eq!(a.method.as_deref(), Some("konto claude.ai"));
        let all = serde_json::to_string(&a).unwrap_or_default();
        assert!(
            !all.contains("ala@example.com") && !all.contains("577ee1b4"),
            "{all}"
        );
        let off = parse_claude(&out(true, r#"{"loggedIn": false}"#));
        assert_eq!(off.state, BridgeAccountState::LoggedOut);
        assert_eq!(
            parse_claude(&out(false, "błąd")).state,
            BridgeAccountState::Error
        );
    }

    #[test]
    fn codex_status_lines() {
        let a = parse_codex(&out(true, "Logged in using ChatGPT\n"));
        assert_eq!(a.state, BridgeAccountState::LoggedIn);
        assert_eq!(a.method.as_deref(), Some("konto ChatGPT"));
        assert_eq!(
            parse_codex(&out(false, "Not logged in")).state,
            BridgeAccountState::LoggedOut
        );
        assert_eq!(
            parse_codex(&out(true, "Logged in using an API key - sk-***"))
                .method
                .as_deref(),
            Some("klucz API")
        );
        assert_eq!(
            parse_codex(&out(true, "???")).state,
            BridgeAccountState::Error
        );
    }

    #[test]
    fn antigravity_and_missing_tools() {
        assert_eq!(status_args(BridgeKind::Antigravity), None);
        assert_eq!(
            check(BridgeKind::Antigravity, None).state,
            BridgeAccountState::Unknown
        );
        assert_eq!(
            check(BridgeKind::Codex, None).state,
            BridgeAccountState::Error
        );
        assert_eq!(
            not_installed(BridgeKind::Codex).state,
            BridgeAccountState::NotInstalled
        );
    }
}
