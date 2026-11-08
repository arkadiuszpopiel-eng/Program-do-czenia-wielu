//! Deterministyczny generator syntetycznego zestawu recall PL (`evals/F7/recall/synthetic/`).
//!
//! Fakt → dwa zapytania (pytanie naturalne, słowa kluczowe); co piąte zapytanie dostaje wariant
//! bez polskich znaków (`fold_pl`, małe litery). Zapytanie przeszukuje zakres faktu i zakres
//! globalny (jak `recall` agentki: sesja/projekt/agentka + globalna). Zmyłki — fakty bez zapytań.

use lib_sqlstore::fold_pl;
use memory_contract::Layer;

use super::data::{DISTRACTORS, GROUPS};
use super::format::{CorpusItem, RecallQuery, RecallSet};

fn layer(name: &str) -> Layer {
    match name {
        "episodic" => Layer::Episodic,
        "procedural" => Layer::Procedural,
        _ => Layer::Semantic,
    }
}

/// Zestaw syntetyczny (zawsze ten sam dla tej samej wersji kodu).
pub fn synthetic_set() -> RecallSet {
    let mut set = RecallSet::default();
    let mut q = 0_usize;
    let mut push_query =
        |set: &mut RecallSet, text: String, kind: &str, fact: &str, scopes: &[String]| {
            q += 1;
            set.queries.push(RecallQuery {
                id: format!("q{q:03}"),
                query: text,
                expected: vec![fact.to_owned()],
                scopes: scopes.to_vec(),
                kind: kind.into(),
            });
            q
        };
    let mut f = 0_usize;
    for group in GROUPS {
        let mut scopes = vec![group.scope.to_owned()];
        if group.scope != "global" {
            scopes.push("global".into());
        }
        for (layer_name, text, queries) in group.facts {
            f += 1;
            let id = format!("f{f:03}");
            set.corpus.push(CorpusItem {
                id: id.clone(),
                scope: group.scope.into(),
                layer: layer(layer_name),
                text: (*text).into(),
                subject: None,
            });
            for (kind, query) in ["pytanie", "slowa_kluczowe"].iter().zip(queries) {
                let n = push_query(&mut set, (*query).into(), kind, &id, &scopes);
                if n % 5 == 0 {
                    let folded = fold_pl(query).to_lowercase();
                    push_query(&mut set, folded, "bez_znakow", &id, &scopes);
                }
            }
        }
    }
    for (i, (scope, text)) in DISTRACTORS.iter().enumerate() {
        set.corpus.push(CorpusItem {
            id: format!("z{:03}", i + 1),
            scope: (*scope).into(),
            layer: Layer::Semantic,
            text: (*text).into(),
            subject: None,
        });
    }
    set
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn synthetic_set_is_valid_deterministic_and_large_enough() {
        let a = synthetic_set();
        assert_eq!(a, synthetic_set());
        a.validate().unwrap();
        assert!(a.is_acceptance_sized(), "zapytań: {}", a.queries.len());
        assert!(a.queries.iter().any(|q| q.kind == "bez_znakow"));
        let (corpus, queries) = a.to_ndjson();
        assert_eq!(RecallSet::parse(&corpus, &queries).unwrap(), a);
    }
}
