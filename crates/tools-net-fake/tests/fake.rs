//! Atrapy `tools-net`: narzędzia przechodzą kontrakt i odrzucają złe adresy bez zapisu
//! wywołania; wirtualna sieć odrzuca adresy niepubliczne i rebinding, loguje żądania.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use tools_common_contract::contract_tests::ctx;
use tools_common_contract::{ToolOutcome, Toolset};
use tools_net_contract::{
    HttpMethod, HttpPort, HttpRequest, NetError, NoSearch, SearchHit, SearchPort,
};
use tools_net_fake::{FakeHttp, FakeRoute, FakeSearch, FakeTools};

#[tokio::test]
async fn contract_and_scripting() {
    let fake = FakeTools::default();
    tools_net_contract::contract_tests::run_all(&fake.tools()).await;
    let before = fake.calls().len();
    let fetch = fake.tools().remove(0);
    fake.push(
        "net_fetch",
        ToolOutcome::ok("Strona", serde_json::json!({})),
    );
    let out = fetch
        .call(serde_json::json!({"url": "https://sklep.pl/"}), &ctx("/"))
        .await;
    assert_eq!(out.text, "Strona");
    assert_eq!(out.untrusted, fetch.manifest().untrusted_output.clone());
    let bad = fetch
        .call(
            serde_json::json!({"url": "https://192.168.1.1/"}),
            &ctx("/"),
        )
        .await;
    assert!(!bad.is_ok());
    assert_eq!(fake.calls().len(), before + 1);
}

fn get(url: &str) -> HttpRequest {
    HttpRequest {
        url: url.into(),
        method: HttpMethod::Get,
    }
}

#[tokio::test]
async fn virtual_network() {
    let net = FakeHttp::default();
    net.page("https://a.pl/", "text/html", b"<p>ok</p>");
    net.route(
        "https://a.pl/r",
        FakeRoute::Redirect {
            status: 302,
            location: "/".into(),
        },
    );
    net.rebind_after("b.pl", 1);
    net.private_host("intra.example");
    let mut r = net.send(&get("https://a.pl/")).await.unwrap();
    assert_eq!(r.body.chunk().await.unwrap().unwrap(), b"<p>ok</p>");
    assert_eq!(r.body.chunk().await.unwrap(), None);
    let r = net.send(&get("https://a.pl/r")).await.unwrap();
    assert_eq!(r.redirect_location(), Some("/"));
    assert_eq!(net.send(&get("https://b.pl/")).await.unwrap().status, 404);
    assert!(matches!(
        net.send(&get("https://b.pl/")).await,
        Err(NetError::Blocked(_))
    ));
    assert!(matches!(
        net.send(&get("https://intra.example/")).await,
        Err(NetError::Blocked(_))
    ));
    assert!(matches!(
        net.send(&get("https://127.0.0.1/")).await,
        Err(NetError::Blocked(_))
    ));
    assert_eq!(net.requests().len(), 3);
    assert_eq!(net.blocked(), vec!["b.pl", "intra.example"]);
    assert!(!net.hosts_contacted().contains("intra.example"));
    let s = FakeSearch::new(
        "api.szukaj.example",
        vec![SearchHit {
            title: "t".into(),
            url: "https://x.pl/".into(),
            snippet: "s".into(),
        }],
    );
    assert_eq!(s.search("q", 5).await.unwrap().len(), 1);
    assert_eq!(s.queries(), vec!["q"]);
    assert_eq!(NoSearch.endpoint_host(), None);
    assert!(NoSearch.search("q", 1).await.is_err());
}
