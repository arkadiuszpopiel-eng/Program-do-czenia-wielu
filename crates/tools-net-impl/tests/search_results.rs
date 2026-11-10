//! Wynik dostawcy też jest niezaufany: adresy, limity i sekrety w projekcji narzędzia.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::sync::Arc;

use async_trait::async_trait;
use common::{Opts, ctx, harness_with};
use serde_json::json;
use tools_net_contract::{NetError, SearchHit, SearchPort};

struct UnboundedSearch(Vec<SearchHit>);

#[async_trait]
impl SearchPort for UnboundedSearch {
    fn endpoint_host(&self) -> Option<String> {
        Some("api.szukaj.example".into())
    }

    async fn search(&self, _: &str, _: u32) -> Result<Vec<SearchHit>, NetError> {
        Ok(self.0.clone()) // Celowo nie przestrzega limitu dostawcy.
    }
}

fn hit(url: &str) -> SearchHit {
    SearchHit {
        title: "Źródło".into(),
        url: url.into(),
        snippet: "Opis źródła — zażółć gęślą jaźń.".into(),
    }
}

#[tokio::test]
async fn urls_with_detected_secrets_never_reach_text_or_json() {
    let h = harness_with(Opts {
        search: Some(Arc::new(UnboundedSearch(vec![
            hit("https://public.example/?token=tajny123"),
            hit("https://public.example/?password=haslo123"),
            hit("https://public.example/bezpieczne"),
        ]))),
        ..Opts::default()
    });
    let out = h
        .tool("net_search")
        .call(json!({"query":"pytanie","max_results":1}), &ctx())
        .await;
    assert!(out.is_ok(), "{}", out.text);
    let serialized = serde_json::to_string(&out).unwrap();
    assert!(!serialized.contains("tajny123"), "sekret z URL ujawniony");
    assert!(!serialized.contains("haslo123"));
    assert_eq!(out.data["results"].as_array().unwrap().len(), 1);
    assert_eq!(
        out.data["results"][0]["url"],
        "https://public.example/bezpieczne"
    );
}

#[tokio::test]
async fn result_count_is_enforced_even_if_provider_ignores_max() {
    let h = harness_with(Opts {
        search: Some(Arc::new(UnboundedSearch(vec![
            hit("https://one.example/"),
            hit("https://two.example/"),
            hit("https://three.example/"),
        ]))),
        ..Opts::default()
    });
    let out = h
        .tool("net_search")
        .call(json!({"query":"pytanie","max_results":1}), &ctx())
        .await;
    assert!(out.is_ok());
    assert_eq!(out.data["results"].as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn malformed_or_non_public_source_addresses_are_omitted() {
    let mut hits = [
        "https://127.0.0.1/a",
        "http://public.example/",
        "javascript:alert(1)",
        "https://localhost/",
        "https://u:pass@public.example/",
        "https://public.example/a\n#fałszywe-źródło",
    ]
    .map(hit)
    .to_vec();
    hits.push(hit("https://public.example/źródło#sekcja"));
    let h = harness_with(Opts {
        search: Some(Arc::new(UnboundedSearch(hits))),
        ..Opts::default()
    });
    let out = h
        .tool("net_search")
        .call(json!({"query":"pytanie"}), &ctx())
        .await;
    assert!(out.is_ok());
    assert_eq!(out.data["results"].as_array().unwrap().len(), 1);
    assert!(out.untrusted.is_some());
    assert!(out.text.contains("pominięto"));
    assert!(h.egress_tokens().contains(&"api.szukaj.example".to_owned()));
}

#[tokio::test]
async fn structured_result_fields_respect_existing_character_budget() {
    let mut source = hit("https://public.example/source");
    source.title = "Ł🙂 ".repeat(1000);
    source.snippet = "token=tajny123 ".repeat(1000);
    let config = tools_net_contract::NetToolsConfig {
        output_max_chars: 512,
        ..Default::default()
    };
    let h = harness_with(Opts {
        search: Some(Arc::new(UnboundedSearch(vec![source]))),
        config,
        ..Opts::default()
    });
    let out = h
        .tool("net_search")
        .call(json!({"query":"pytanie"}), &ctx())
        .await;
    assert!(out.is_ok());
    let values = out.data["results"].as_array().unwrap();
    assert_eq!(values.len(), 1);
    let chars: usize = values
        .iter()
        .flat_map(|v| ["url", "title", "snippet"].map(|f| v[f].as_str().unwrap().chars().count()))
        .sum();
    assert!(chars <= 512, "przekroczony budżet pól JSON:{chars}");
    assert!(!serde_json::to_string(&out).unwrap().contains("tajny123"));
}

#[tokio::test]
async fn empty_provider_result_differs_from_discarded_sources() {
    for (hits, discarded) in [
        (vec![], false),
        (vec![hit("https://public.example/?token=tajny123")], true),
    ] {
        let h = harness_with(Opts {
            search: Some(Arc::new(UnboundedSearch(hits))),
            ..Opts::default()
        });
        let out = h
            .tool("net_search")
            .call(json!({"query":"pytanie"}), &ctx())
            .await;
        assert!(out.is_ok());
        assert!(out.data["results"].as_array().unwrap().is_empty());
        assert_eq!(out.text.contains("pominięto"), discarded);
        assert!(!serde_json::to_string(&out).unwrap().contains("tajny123"));
    }
}

#[tokio::test]
async fn zero_and_small_budgets_never_shorten_source_addresses() {
    for budget in [0, 1, 22, 64, 512] {
        let url = "https://public.example/";
        let mut source = hit(url);
        source.title = "Ł🙂".repeat(100);
        let config = tools_net_contract::NetToolsConfig {
            output_max_chars: budget,
            ..Default::default()
        };
        let h = harness_with(Opts {
            search: Some(Arc::new(UnboundedSearch(vec![source]))),
            config,
            ..Opts::default()
        });
        let out = h
            .tool("net_search")
            .call(json!({"query":"pytanie"}), &ctx())
            .await;
        assert!(out.is_ok());
        let values = out.data["results"].as_array().unwrap();
        let chars: usize = values
            .iter()
            .flat_map(|v| {
                ["url", "title", "snippet"].map(|f| v[f].as_str().unwrap().chars().count())
            })
            .sum();
        assert!(chars <= budget);
        for value in values {
            assert_eq!(value["url"], url);
        }
        if budget < url.chars().count() {
            assert!(values.is_empty());
        }
    }
}

async fn projected(
    hits: Vec<SearchHit>,
    max: u32,
    budget: usize,
) -> tools_common_contract::ToolOutcome {
    let h = harness_with(Opts {
        search: Some(Arc::new(UnboundedSearch(hits))),
        config: tools_net_contract::NetToolsConfig {
            output_max_chars: budget,
            ..Default::default()
        },
        ..Opts::default()
    });
    let out = h
        .tool("net_search")
        .call(json!({"query":"źródła","max_results":max}), &ctx())
        .await;
    assert!(out.is_ok(), "{}", out.text);
    out
}

fn urls(out: &tools_common_contract::ToolOutcome) -> Vec<&str> {
    out.data["results"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v["url"].as_str().unwrap())
        .collect()
}

#[tokio::test]
async fn duplicate_does_not_hide_next_source_at_result_limit() {
    let a = "https://docs.example/a";
    let b = "https://docs.example/b";
    let out = projected(vec![hit(a), hit(a), hit(b)], 2, 4096).await;
    assert_eq!(urls(&out), vec![a, b]);
    assert!(out.text.contains("powtórzenia"), "{}", out.text);
    assert!(out.untrusted.is_some());
}

#[tokio::test]
async fn canonical_target_dedup_keeps_first_original_source_and_rank() {
    let first = "HTTPS://DOCS.EXAMPLE:443/guide#start";
    let mut source = hit(first);
    source.title = "Pierwszy tytuł".into();
    source.snippet = "Pierwszy opis".into();
    let out = projected(
        vec![
            source,
            hit("https://docs.example/guide#next"),
            hit("https://docs.example/guide"),
            hit("https://other.example/"),
        ],
        10,
        4096,
    )
    .await;
    assert_eq!(urls(&out), vec![first, "https://other.example/"]);
    assert_eq!(out.data["results"][0]["title"], "Pierwszy tytuł");
    assert_eq!(out.data["results"][0]["snippet"], "Pierwszy opis");
}

#[tokio::test]
async fn duplicate_title_does_not_spend_budget_for_other_source() {
    let a = "https://docs.example/a";
    let b = "https://docs.example/b";
    let mut duplicate = hit(a);
    duplicate.title = "Duplikat ".repeat(1000);
    let out = projected(vec![hit(a), duplicate, hit(b)], 10, 256).await;
    assert_eq!(urls(&out), vec![a, b]);
    assert_eq!(out.data["results"][1]["snippet"], hit(b).snippet);
}

#[tokio::test]
async fn path_case_query_and_nondefault_port_remain_distinct() {
    let sources = [
        "https://docs.example/A",
        "https://docs.example/a",
        "https://docs.example/a?page=1",
        "https://docs.example/a?page=2",
        "https://docs.example:8443/a",
    ];
    let out = projected(sources.into_iter().map(hit).collect(), 10, 4096).await;
    assert_eq!(urls(&out), sources);
    assert!(!out.text.contains("pominięto"));
}

#[tokio::test]
async fn secret_fragment_is_rejected_before_recording_seen_target() {
    let safe = "https://docs.example/a#public";
    let out = projected(
        vec![
            hit("https://docs.example/a#token=tajny123"),
            hit(safe),
            hit("https://docs.example/a#other"),
        ],
        10,
        4096,
    )
    .await;
    assert_eq!(urls(&out), vec![safe]);
    assert!(!serde_json::to_string(&out).unwrap().contains("tajny123"));
}

#[tokio::test]
async fn percent_encoded_parameter_names_do_not_expose_detected_secrets() {
    for key in [
        "%74%6f%6b%65%6e",
        "%70%61%73%73%77%6f%72%64",
        "%61%70%69%5f%6b%65%79",
    ] {
        let unsafe_url = format!("https://docs.example/a?{key}=tajny123");
        let safe = "https://docs.example/safe";
        let out = projected(vec![hit(&unsafe_url), hit(safe)], 10, 4096).await;
        assert_eq!(urls(&out), vec![safe], "{key}");
        assert!(!serde_json::to_string(&out).unwrap().contains("tajny123"));
        assert!(out.text.contains("pominięto"));
    }
}

#[tokio::test]
async fn encoded_known_secret_prefix_in_query_value_is_omitted() {
    let safe = "https://docs.example/safe";
    let out = projected(
        vec![
            hit("https://docs.example/a?ref=%73%6b%2DABCDEFGHIJKLMNOPQRSTUV"),
            hit(safe),
        ],
        10,
        4096,
    )
    .await;
    assert_eq!(urls(&out), vec![safe]);
    assert!(
        !serde_json::to_string(&out)
            .unwrap()
            .contains("ABCDEFGHIJKLMNOPQRSTUV")
    );
}

#[tokio::test]
async fn harmless_encoded_query_is_preserved_without_rewriting_source_url() {
    let sources = [
        "https://docs.example/a?q=za%C5%BC%C3%B3%C5%82%C4%87",
        "https://docs.example/a?query=tokenizacja+tekstu&lang=pl",
        "https://docs.example/a?query=abc%3D123",
    ];
    let out = projected(sources.into_iter().map(hit).collect(), 10, 4096).await;
    assert_eq!(urls(&out), sources);
    assert!(!out.text.contains("pominięto"));
}
