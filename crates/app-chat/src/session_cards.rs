//! Karty rozmów z profilem (P4, widok „Karty” panelu Sesje): dla każdej rozmowy agentki, które
//! odpowiadały (bez odpowiedzi — obsada sesji), ostatnia widoczna wiadomość jako czysty tekst,
//! model ostatniej odpowiedzi i łączny koszt. Dane liczone z historii sesji (`SessionHistory`) —
//! `SessionSummary` zostaje bez zmian.
//!
//! Koszt: suma mikro-USD wszystkich gałęzi przeliczona kursem `usd_pln_e4` (USD→PLN × 10⁴, ten sam
//! co w szczegółach kosztów) na grosze; bez kursu (0) — centy USD.
//!
//! Pamięć i dysk: każda czytana rozmowa otwiera swoją zaszyfrowaną bazę. Po odczycie wywołujemy
//! `release(id)` — `app-core` zamyka tam bazę, której nie było otwartej przed wywołaniem
//! (`SqliteSessions::close_session`), żeby widok „Karty” nie trzymał otwartych wszystkich baz.
//! UI prosi tylko o rozmowy nowe albo zmienione (`updated_at`), partiami po [`MAX_CARDS`].
//! Wywołanie jest synchroniczne — komenda IPC uruchamia je w `spawn_blocking`.
//!
//! Treść ostatniej wiadomości przechodzi przez `lib_markdown::to_spoken_text` (bez znaczników,
//! kod i tabele jako krótka fraza), a potem traci znaki sterujące i nadmiar spacji — UI wstawia ją
//! jako zwykły tekst, nigdy jako HTML.

use std::collections::BTreeSet;

use app_api::dto::{Currency, Money, SessionCard};
use sessions_contract::{Author, Role, SessionId, Sessions, Turn};

/// Najwięcej kart w jednym wywołaniu (dalsze identyfikatory są pomijane).
pub const MAX_CARDS: usize = 200;
/// Najdłuższy fragment ostatniej wiadomości (znaki).
pub const SNIPPET_CHARS: usize = 160;
/// Najwięcej agentek na karcie.
pub const MAX_AGENTS: usize = 4;

/// Karty dla podanych rozmów, w kolejności identyfikatorów (bez powtórzeń, do [`MAX_CARDS`]).
/// Rozmowa nieistniejąca albo w koszu jest pomijana; nieczytelna historia daje kartę bez
/// fragmentu, modelu i kosztu (obsada z metadanych zostaje). `usd_pln_e4` — kurs USD→PLN × 10⁴
/// (0 — koszt w USD); `release` — wołane po odczycie historii każdej rozmowy (zamknięcie bazy).
pub fn session_cards(
    store: &dyn Sessions,
    ids: &[String],
    usd_pln_e4: u64,
    release: &dyn Fn(&SessionId),
) -> Vec<SessionCard> {
    let mut seen = BTreeSet::new();
    let mut out = Vec::new();
    for raw in ids {
        let raw = raw.trim();
        if raw.is_empty() || !seen.insert(raw.to_owned()) {
            continue;
        }
        if out.len() >= MAX_CARDS {
            break;
        }
        let id = SessionId::new(raw);
        let Ok(meta) = store.session(&id) else {
            continue;
        };
        if meta.trashed {
            continue;
        }
        let mut agents: Vec<String> = Vec::new();
        let mut card = SessionCard {
            session_id: raw.to_owned(),
            agents: Vec::new(),
            snippet: None,
            snippet_author: None,
            model: None,
            cost: None,
        };
        let visible = visible_line(store, &id);
        match store.all_turns(&id) {
            Ok(all) => {
                card.cost = total_cost(&all, usd_pln_e4);
                for turn in &all {
                    if let Author::Agent { agent } = &turn.author {
                        push_agent(&mut agents, agent.as_str());
                    }
                }
            }
            Err(e) => tracing::warn!(session = raw, error = %e, "karta rozmowy: brak historii"),
        }
        // Nikt jeszcze nie odpowiedział — karta pokazuje obsadę sesji.
        if agents.is_empty() {
            for agent in &meta.agents {
                push_agent(&mut agents, agent.as_str());
            }
        }
        release(&id);
        card.model = last_model(&visible);
        if let Some((text, author)) = last_message(&visible) {
            card.snippet = Some(text);
            card.snippet_author = Some(author);
        }
        card.agents = agents;
        out.push(card);
    }
    out
}

fn push_agent(agents: &mut Vec<String>, agent: &str) {
    let agent = agent.trim();
    if !agent.is_empty() && agents.len() < MAX_AGENTS && !agents.iter().any(|a| a == agent) {
        agents.push(agent.to_owned());
    }
}

/// Aktywna linia rozmowy (od korzenia do wyświetlanego liścia) bez tur ukrytych.
fn visible_line(store: &dyn Sessions, id: &SessionId) -> Vec<Turn> {
    let Ok(Some(leaf)) = store.active_leaf(id) else {
        return Vec::new();
    };
    store
        .branch_projection(id, leaf)
        .map(|line| line.into_iter().filter(|t| !t.hidden).collect())
        .unwrap_or_default()
}

/// Model ostatniej odpowiedzi na aktywnej linii (wybrany wariant odpowiedzi, nie najnowsza gałąź).
fn last_model(line: &[Turn]) -> Option<String> {
    line.iter().rev().find_map(|turn| {
        let model = turn.usage.as_ref()?.model.trim();
        (turn.role == Role::Assistant && !model.is_empty()).then(|| model.to_owned())
    })
}

/// Suma kosztów wszystkich gałęzi (wydane pieniądze nie znikają po wybraniu innego wariantu
/// odpowiedzi), w groszach po kursie `usd_pln_e4` albo — bez kursu — w centach USD; zaokrąglona
/// w górę.
fn total_cost(all: &[Turn], usd_pln_e4: u64) -> Option<Money> {
    let micro_usd: u64 = all
        .iter()
        .filter_map(|turn| turn.usage.as_ref())
        .fold(0, |sum, usage| sum.saturating_add(usage.cost.micro_usd));
    if micro_usd == 0 {
        return None;
    }
    let (minor, currency) = if usd_pln_e4 > 0 {
        // mikro-PLN = mikro-USD × kurs × 10⁴ / 10⁴; grosz = 10⁴ mikro-PLN.
        let micro_pln = u128::from(micro_usd) * u128::from(usd_pln_e4) / 10_000;
        (micro_pln.div_ceil(10_000), Currency::Pln)
    } else {
        (u128::from(micro_usd).div_ceil(10_000), Currency::Usd)
    };
    Some(Money {
        minor: i64::try_from(minor).unwrap_or(i64::MAX),
        currency,
    })
}

/// Ostatnia wiadomość użytkownika albo agentki z niepustym tekstem.
fn last_message(line: &[Turn]) -> Option<(String, String)> {
    line.iter().rev().find_map(|turn| {
        let author = match (&turn.role, &turn.author) {
            (Role::User, _) => "user".to_owned(),
            (Role::Assistant, Author::Agent { agent }) => agent.as_str().to_owned(),
            _ => return None,
        };
        let text = plain_snippet(&turn.content.searchable_text());
        (!text.is_empty()).then_some((text, author))
    })
}

/// Czysty tekst z Markdownu: bez znaczników, znaków sterujących i nadmiaru spacji, do
/// [`SNIPPET_CHARS`] znaków (dłuższy kończy się wielokropkiem).
pub fn plain_snippet(markdown: &str) -> String {
    let spoken = lib_markdown::to_spoken_text(markdown);
    let cleaned: String = spoken
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .filter(|c| !is_invisible(*c))
        .collect();
    let words: Vec<&str> = cleaned.split_whitespace().collect();
    let text = words.join(" ");
    if text.chars().count() <= SNIPPET_CHARS {
        return text;
    }
    let cut: String = text.chars().take(SNIPPET_CHARS - 1).collect();
    format!("{}…", cut.trim_end())
}

/// Znaki niewidoczne i sterujące kierunkiem tekstu (podmiana kolejności, „pusty” tekst).
fn is_invisible(c: char) -> bool {
    matches!(
        c,
        '\u{200B}'..='\u{200F}' | '\u{202A}'..='\u{202E}' | '\u{2060}'..='\u{2069}' | '\u{FEFF}'
    )
}

#[cfg(test)]
#[path = "session_cards_tests.rs"]
mod tests;
