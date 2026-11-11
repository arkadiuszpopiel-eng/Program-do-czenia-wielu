//! Stop ustawiony w ostatnim poll dostawcy ma być zauważony przed oddaniem wyniku.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
use std::time::Duration;

use async_trait::async_trait;
use common::{Opts, ctx, harness_with};
use serde_json::json;
use tokio::sync::Notify;
use tools_common_contract::{ToolOutcome, ToolStatus};
use tools_net_contract::{NetError, SearchHit, SearchPort};

struct Reply {
    on_reply: Arc<dyn Fn() + Send + Sync>,
    calls: Arc<AtomicUsize>,
    fail: bool,
}

#[async_trait]
impl SearchPort for Reply {
    fn endpoint_host(&self) -> Option<String> {
        Some("api.szukaj.example".into())
    }
    async fn search(&self, _: &str, _: u32) -> Result<Vec<SearchHit>, NetError> {
        self.calls.fetch_add(1, Ordering::Relaxed);
        (self.on_reply)();
        if self.fail {
            Err(NetError::Connect("awaria dostawcy".into()))
        } else {
            Ok(vec![SearchHit {
                title: "Wynik".into(),
                url: "https://source.example/".into(),
                snippet: "Odrzucona treść".into(),
            }])
        }
    }
}

async fn reply(cancel_on_reply: bool, fail: bool) -> ToolOutcome {
    let c = ctx();
    let cancel = c.cancel.clone();
    let calls = Arc::new(AtomicUsize::new(0));
    let h = harness_with(Opts {
        search: Some(Arc::new(Reply {
            on_reply: Arc::new(move || {
                if cancel_on_reply {
                    cancel.cancel();
                }
            }),
            calls: calls.clone(),
            fail,
        })),
        ..Default::default()
    });
    let out = h
        .tool("net_search")
        .call(json!({"query":"źródła"}), &c)
        .await;
    assert_eq!(
        calls.load(Ordering::Relaxed),
        1,
        "Dostawca faktycznie został wywołany"
    );
    assert_eq!(c.cancel.is_cancelled(), cancel_on_reply);
    assert_eq!(h.egress_tokens(), vec!["api.szukaj.example"]);
    out
}

fn assert_cancelled(out: &ToolOutcome) {
    assert_eq!(out.status, ToolStatus::Cancelled, "{out:#?}");
    assert!(out.data.is_null());
    assert!(!out.text.contains("Odrzucona treść"));
    assert!(out.text.contains("mogło już dotrzeć"), "{out:#?}");
    assert!(!out.text.contains("nie wykonano"));
}

#[tokio::test]
async fn cancellation_set_by_successful_reply_does_not_publish_success() {
    assert_cancelled(&reply(true, false).await);
}

#[tokio::test]
async fn cancellation_set_by_failed_reply_does_not_replace_stop_with_io_error() {
    assert_cancelled(&reply(true, true).await);
}

#[tokio::test]
async fn success_and_provider_error_without_stop_keep_original_statuses() {
    let ok = reply(false, false).await;
    assert!(ok.is_ok());
    assert_eq!(ok.data["results"][0]["url"], "https://source.example/");
    let error = reply(false, true).await;
    assert!(matches!(error.status, ToolStatus::Failed { .. }));
    assert!(error.text.contains("awaria dostawcy"));
}

struct Hanging(Arc<Notify>);

#[async_trait]
impl SearchPort for Hanging {
    fn endpoint_host(&self) -> Option<String> {
        Some("api.szukaj.example".into())
    }
    async fn search(&self, _: &str, _: u32) -> Result<Vec<SearchHit>, NetError> {
        self.0.notify_one();
        std::future::pending().await // Celowo ignoruje token.
    }
}

#[tokio::test(start_paused = true)]
async fn pending_provider_is_cancelled_without_claiming_query_was_never_sent() {
    let entered = Arc::new(Notify::new());
    let h = harness_with(Opts {
        search: Some(Arc::new(Hanging(entered.clone()))),
        ..Default::default()
    });
    let c = ctx();
    let tool = h.tool("net_search");
    let (out, ()) = tokio::time::timeout(Duration::from_secs(1), async {
        tokio::join!(tool.call(json!({"query":"źródła"}), &c), async {
            entered.notified().await;
            c.cancel.cancel();
        })
    })
    .await
    .expect("Stop kończy oczekiwanie na ignorujący token port");
    assert_cancelled(&out);
    assert_eq!(h.egress_tokens(), vec!["api.szukaj.example"]);
}
