//! Testy kart rozmów: obsada, ostatnia wiadomość jako czysty tekst, model, koszt, pomijanie.

use core_bus_contract::Cost;
use sessions_contract::{
    AgentId, ModelUsage, NewSession, NewTurn, SessionCatalog, SessionHistory, SessionId,
};
use sessions_fake::FakeSessions;

use super::*;

fn usage(model: &str, micro_usd: u64) -> Option<ModelUsage> {
    Some(ModelUsage {
        provider: "test".into(),
        model: model.into(),
        cost: Cost {
            input_tokens: 10,
            output_tokens: 20,
            micro_usd,
            latency_ms: None,
        },
    })
}

/// Karty bez kursu (koszt w USD) i bez zamykania baz.
fn cards(store: &FakeSessions, ids: &[String]) -> Vec<SessionCard> {
    session_cards(store, ids, 0, &|_| {})
}

fn session(store: &FakeSessions, agents: &[&str]) -> SessionId {
    store
        .create_session(NewSession {
            title: "Karta".into(),
            agents: agents.iter().map(|a| AgentId::new(*a)).collect(),
            ..NewSession::default()
        })
        .unwrap()
        .id
}

#[test]
fn card_has_agents_snippet_model_and_cost() {
    let store = FakeSessions::new();
    let id = session(&store, &["alfa"]);
    let q = store
        .append_turn(&id, None, NewTurn::user("Jak **działa** łódź?"))
        .unwrap();
    let mut answer = NewTurn::assistant("beta", "Łódź `pływa`, bo [wypiera](https://x.pl) wodę.");
    answer.usage = usage("claude-test", 12_345);
    store.append_turn(&id, Some(q.id), answer).unwrap();

    let cards = cards(&store, &[id.to_string()]);
    assert_eq!(cards.len(), 1);
    let card = &cards[0];
    assert_eq!(card.session_id, id.to_string());
    // Obsada sesji ustępuje agentkom, które odpowiadały.
    assert_eq!(card.agents, vec!["beta".to_owned()]);
    assert_eq!(
        card.snippet.as_deref(),
        Some("Łódź pływa, bo wypiera wodę.")
    );
    assert_eq!(card.snippet_author.as_deref(), Some("beta"));
    assert_eq!(card.model.as_deref(), Some("claude-test"));
    assert_eq!(
        card.cost,
        Some(Money {
            minor: 2,
            currency: Currency::Usd
        })
    );
}

#[test]
fn cost_counts_every_branch_and_snippet_follows_active_line() {
    let store = FakeSessions::new();
    let id = session(&store, &[]);
    let q = store
        .append_turn(&id, None, NewTurn::user("Pytanie"))
        .unwrap();
    let mut first = NewTurn::assistant("alfa", "Pierwsza odpowiedź");
    first.usage = usage("m1", 10_000);
    let first = store.append_turn(&id, Some(q.id), first).unwrap();
    let mut second = NewTurn::assistant("gama", "Druga odpowiedź");
    second.usage = usage("m2", 20_000);
    store.fork_from(&id, first.id, second).unwrap();

    let card = &cards(&store, &[id.to_string()])[0];
    assert_eq!(card.snippet.as_deref(), Some("Druga odpowiedź"));
    assert_eq!(card.model.as_deref(), Some("m2"));
    assert_eq!(card.cost.map(|m| m.minor), Some(3));
    assert_eq!(card.agents, vec!["alfa".to_owned(), "gama".to_owned()]);

    // Powrót do starszego wariantu: model i fragment z aktywnej linii, koszt nadal z obu gałęzi.
    store.set_active_leaf(&id, first.id).unwrap();
    let card = &cards(&store, &[id.to_string()])[0];
    assert_eq!(card.snippet.as_deref(), Some("Pierwsza odpowiedź"));
    assert_eq!(card.model.as_deref(), Some("m1"));
    assert_eq!(card.cost.map(|m| m.minor), Some(3));
}

#[test]
fn cost_in_pln_by_rate_and_release_after_each_read() {
    let store = FakeSessions::new();
    let id = session(&store, &[]);
    let q = store
        .append_turn(&id, None, NewTurn::user("Pytanie"))
        .unwrap();
    let mut a = NewTurn::assistant("alfa", "Odpowiedź");
    a.usage = usage("m", 12_345);
    store.append_turn(&id, Some(q.id), a).unwrap();
    let trashed = session(&store, &[]);
    store.trash_session(&trashed).unwrap();

    let released = std::cell::RefCell::new(Vec::new());
    let ids = vec![id.to_string(), trashed.to_string(), "nie-ma".into()];
    let out = session_cards(&store, &ids, 36_400, &|s| {
        released.borrow_mut().push(s.to_string());
    });
    // 0,012345 USD × 3,64 = 0,0449 PLN → 5 groszy (w górę).
    assert_eq!(
        out[0].cost,
        Some(Money {
            minor: 5,
            currency: Currency::Pln
        })
    );
    // Zamykana tylko baza rozmowy, której historię czytano.
    assert_eq!(released.into_inner(), vec![id.to_string()]);
}

#[test]
fn local_model_and_empty_session_have_no_cost_or_snippet() {
    let store = FakeSessions::new();
    let empty = session(&store, &["delta"]);
    let local = session(&store, &[]);
    let q = store
        .append_turn(&local, None, NewTurn::user("Hej"))
        .unwrap();
    let mut a = NewTurn::assistant("alfa", "Odpowiedź lokalna");
    a.usage = usage("qwen-local", 0);
    store.append_turn(&local, Some(q.id), a).unwrap();

    let cards = cards(&store, &[empty.to_string(), local.to_string()]);
    assert_eq!(cards[0].snippet, None);
    assert_eq!(cards[0].agents, vec!["delta".to_owned()]);
    assert_eq!(cards[0].cost, None);
    assert_eq!(cards[1].snippet.as_deref(), Some("Odpowiedź lokalna"));
    assert_eq!(cards[1].snippet_author.as_deref(), Some("alfa"));
    assert_eq!(cards[1].model.as_deref(), Some("qwen-local"));
    assert_eq!(cards[1].cost, None);
}

#[test]
fn hidden_turns_are_skipped() {
    let store = FakeSessions::new();
    let id = session(&store, &[]);
    let q = store
        .append_turn(&id, None, NewTurn::user("Widoczne"))
        .unwrap();
    let a = store
        .append_turn(&id, Some(q.id), NewTurn::assistant("alfa", "Ukryte"))
        .unwrap();
    store.set_hidden(&id, a.id, true).unwrap();
    let card = &cards(&store, &[id.to_string()])[0];
    assert_eq!(card.snippet.as_deref(), Some("Widoczne"));
    assert_eq!(card.snippet_author.as_deref(), Some("user"));
}

#[test]
fn unknown_trashed_and_duplicate_ids_are_skipped() {
    let store = FakeSessions::new();
    let kept = session(&store, &[]);
    let trashed = session(&store, &[]);
    store.trash_session(&trashed).unwrap();
    let ids = vec![
        kept.to_string(),
        " ".into(),
        "nie-ma-takiej".into(),
        trashed.to_string(),
        kept.to_string(),
    ];
    let cards = cards(&store, &ids);
    assert_eq!(cards.len(), 1);
    assert_eq!(cards[0].session_id, kept.to_string());
}

#[test]
fn at_most_max_cards() {
    let store = FakeSessions::new();
    let ids: Vec<String> = (0..MAX_CARDS + 3)
        .map(|_| session(&store, &[]).to_string())
        .collect();
    assert_eq!(cards(&store, &ids).len(), MAX_CARDS);
}

#[test]
fn snippet_is_plain_single_line_and_bounded() {
    assert_eq!(
        plain_snippet("# Tytuł\n\n- punkt\n- drugi"),
        "Tytuł. punkt drugi"
    );
    assert_eq!(
        plain_snippet("<script>alert(1)</script>Tekst\u{202E}odwrócony\u{200B}"),
        "alert(1)Tekstodwrócony"
    );
    assert_eq!(plain_snippet("a\tb\u{0007}c"), "a b c");
    let long = "słowo ".repeat(100);
    let out = plain_snippet(&long);
    assert_eq!(out.chars().count(), SNIPPET_CHARS);
    assert!(out.ends_with('…'));
    assert!(!out.contains('\n'));
}

#[test]
fn agents_are_capped_and_deduplicated() {
    let store = FakeSessions::new();
    let id = session(&store, &["alfa", "alfa", "beta", "gama", "delta", "inna"]);
    let card = &cards(&store, &[id.to_string()])[0];
    assert_eq!(card.agents, vec!["alfa", "beta", "gama", "delta"]);
}
