//! Teksty powodów po polsku (eskalacje, raport).

pub(super) fn reason_pl(reason: &str) -> &'static str {
    match reason {
        "resources" => "zajęte zasoby",
        "reserved" => "zasoby zarezerwowane dla ważniejszego zadania",
        "no_agent" => "brak agentki z wymaganą rolą",
        "agent_busy" => "agentki zajęte",
        "concurrency" => "limit równoległości",
        "chain_too_deep" => "za długi łańcuch wyzwalaczy",
        "self_loop" => "wyzwalacz reagowałby na własne zadanie",
        "global_rate_limited" => "globalny limit wyzwoleń",
        _ => "inny powód",
    }
}

pub(super) fn budget_pl(b: &str) -> &'static str {
    match b {
        "steps" => "kroków",
        "wall" => "czasu",
        "cost" => "kosztu",
        _ => "zadania",
    }
}
