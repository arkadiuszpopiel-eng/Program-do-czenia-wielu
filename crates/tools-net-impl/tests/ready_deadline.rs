//! Gotowy wynik portu po terminie nie może ominąć istniejącego limitu czasu.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use async_trait::async_trait;
use compliance_contract::{DenyLists, PathEnv};
use serde_json::json;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Notify;
use tools_common_contract::{ToolErrorKind, ToolOutcome, ToolStatus};
use tools_net_contract::{
    BodyReader, HttpPort, HttpRequest, HttpResponse, NetError, NetToolsConfig, SearchHit,
    SearchPort,
};
use tools_net_impl::{NetTools, NetToolsDeps};

#[derive(Clone, Copy, PartialEq)]
enum At {
    Headers,
    Chunk,
    Eof,
    Search,
}

#[derive(Default)]
struct Gate {
    entered: Notify,
    ready: Notify,
}
impl Gate {
    async fn wait(&self) {
        self.entered.notify_one();
        self.ready.notified().await;
    }
}

struct Http {
    at: At,
    gate: Arc<Gate>,
    fail: bool,
}
struct Body {
    at: At,
    gate: Arc<Gate>,
    sent: bool,
}
#[async_trait]
impl HttpPort for Http {
    async fn send(&self, _: &HttpRequest) -> Result<HttpResponse, NetError> {
        if self.at == At::Headers {
            self.gate.wait().await;
        }
        if self.fail {
            return Err(NetError::Connect("spóźniony błąd".into()));
        }
        Ok(HttpResponse {
            status: 200,
            content_type: Some("text/plain".into()),
            content_length: None,
            location: None,
            content_disposition: None,
            body: Box::new(Body {
                at: self.at,
                gate: self.gate.clone(),
                sent: false,
            }),
        })
    }
}
#[async_trait]
impl BodyReader for Body {
    async fn chunk(&mut self) -> Result<Option<Vec<u8>>, NetError> {
        let at = if self.sent { At::Eof } else { At::Chunk };
        if self.at == at {
            self.gate.wait().await;
        }
        if self.sent {
            Ok(None)
        } else {
            self.sent = true;
            Ok(Some(b"late-body".to_vec()))
        }
    }
}

struct Search {
    gate: Arc<Gate>,
    fail: bool,
}
#[async_trait]
impl SearchPort for Search {
    fn endpoint_host(&self) -> Option<String> {
        Some("search.example".into())
    }
    async fn search(&self, _: &str, _: u32) -> Result<Vec<SearchHit>, NetError> {
        self.gate.wait().await;
        if self.fail {
            Err(NetError::Read("spóźniony błąd".into()))
        } else {
            Ok(vec![SearchHit {
                url: "https://source.example/".into(),
                title: "late-body".into(),
                snippet: "late-body".into(),
            }])
        }
    }
}

async fn late_reply(at: At, fail: bool) -> ToolOutcome {
    let h = common::harness();
    let c = common::ctx();
    let gate = Arc::new(Gate::default());
    let tools = NetTools::new(NetToolsDeps {
        http: Arc::new(Http {
            at,
            gate: gate.clone(),
            fail,
        }),
        search: Some(Arc::new(Search {
            gate: gate.clone(),
            fail,
        })),
        downloads: Arc::new(h.downloads.clone()),
        quarantine_root: None,
        broker: h.broker.clone(),
        deny: DenyLists::baseline(),
        env: PathEnv::windows_profile(&common::home()),
        config: NetToolsConfig {
            max_fetch_bytes: if at == At::Chunk { 1 } else { 2048 },
            ..Default::default()
        },
        bus: Some(h.bus.clone()),
    });
    use tools_common_contract::Toolset;
    let name = if at == At::Search {
        "net_search"
    } else {
        "net_fetch"
    };
    let tool = tools
        .tools()
        .into_iter()
        .find(|t| t.manifest().name == name)
        .unwrap();
    let args = if at == At::Search {
        json!({"query":"źródła"})
    } else {
        json!({"url":"https://files.example/a.txt", "method":if at == At::Headers {"head"} else {"get"}})
    };
    let mut call = Box::pin(tool.call(args, &c));
    tokio::select! {
        out = call.as_mut() => panic!("Przedwczesny wynik: {out:?}"),
        () = gate.entered.notified() => {},
        () = tokio::time::sleep(Duration::from_secs(5)) => panic!("Watchdog wejścia do portu"),
    }
    // Call nie jest spawnowany: podczas skoku zegara nikt go nie odpytuje.
    tokio::time::pause();
    tokio::time::advance(Duration::from_millis(
        NetToolsConfig::default().fetch_timeout_ms + 1,
    ))
    .await;
    gate.ready.notify_one();
    let out = call.await;
    tokio::time::resume();
    out
}

fn timeout(out: ToolOutcome) {
    assert_eq!(
        out.status,
        ToolStatus::Failed {
            error: ToolErrorKind::Timeout
        },
        "{out:#?}"
    );
    assert!(out.data.is_null());
    assert!(!out.text.contains("late-body"));
    assert!(!out.truncated);
}

#[tokio::test]
async fn ready_head_after_deadline_is_timeout() {
    timeout(late_reply(At::Headers, false).await);
}
#[tokio::test]
async fn ready_header_error_after_deadline_is_timeout() {
    timeout(late_reply(At::Headers, true).await);
}
#[tokio::test]
async fn late_chunk_is_timeout_not_size_truncation() {
    timeout(late_reply(At::Chunk, false).await);
}
#[tokio::test]
async fn late_eof_does_not_publish_completed_body() {
    timeout(late_reply(At::Eof, false).await);
}
#[tokio::test]
async fn late_search_success_is_timeout() {
    timeout(late_reply(At::Search, false).await);
}
#[tokio::test]
async fn late_search_error_is_timeout() {
    timeout(late_reply(At::Search, true).await);
}
