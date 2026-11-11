//! Granice Stop: gotowe nagłówki/chunk/EOF oraz zapis przed i po commit pobrania.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use async_trait::async_trait;
use compliance_contract::{DenyLists, PathEnv};
use platform_apps_contract::{DownloadError, DownloadSink, DownloadStore};
use platform_apps_fake::FakeDownloads;
use serde_json::{Value, json};
use std::path::Path;
use std::sync::Arc;
use tools_common_contract::{ToolCtx, ToolOutcome, ToolStatus};
use tools_net_contract::{
    BodyReader, HttpPort, HttpRequest, HttpResponse, NetError, NetToolsConfig,
};
use tools_net_impl::{NetTools, NetToolsDeps};

const URL: &str = "https://files.example/sample.txt";
const BODY: &[u8] = b"server-body";
type Stop = Arc<dyn Fn() + Send + Sync>;

#[derive(Clone, Copy, PartialEq)]
enum At {
    Never,
    Headers,
    Chunk,
    Eof,
    Flush,
    Commit,
}

struct Http {
    at: At,
    stop: Stop,
    fail: bool,
}
struct Body {
    at: At,
    stop: Stop,
    sent: bool,
}

#[async_trait]
impl HttpPort for Http {
    async fn send(&self, _: &HttpRequest) -> Result<HttpResponse, NetError> {
        if self.at == At::Headers {
            (self.stop)();
        }
        if self.fail {
            return Err(NetError::Connect("test-connect".into()));
        }
        Ok(HttpResponse {
            status: 200,
            content_type: Some("text/plain".into()),
            content_length: None,
            location: None,
            content_disposition: None,
            body: Box::new(Body {
                at: self.at,
                stop: self.stop.clone(),
                sent: false,
            }),
        })
    }
}

#[async_trait]
impl BodyReader for Body {
    async fn chunk(&mut self) -> Result<Option<Vec<u8>>, NetError> {
        if !self.sent {
            self.sent = true;
            if self.at == At::Chunk {
                (self.stop)();
            }
            Ok(Some(BODY.to_vec()))
        } else {
            if self.at == At::Eof {
                (self.stop)();
            }
            Ok(None)
        }
    }
}

struct Store {
    base: FakeDownloads,
    at: At,
    stop: Stop,
}
struct Sink {
    base: Box<dyn DownloadSink>,
    at: At,
    stop: Stop,
}

impl DownloadStore for Store {
    fn begin(&self, dir: &Path, name: &str) -> Result<Box<dyn DownloadSink>, DownloadError> {
        Ok(Box::new(Sink {
            base: self.base.begin(dir, name)?,
            at: self.at,
            stop: self.stop.clone(),
        }))
    }
}
impl DownloadSink for Sink {
    fn write(&mut self, bytes: &[u8]) -> Result<(), DownloadError> {
        self.base.write(bytes)?;
        if self.at == At::Flush {
            (self.stop)();
        }
        Ok(())
    }
    fn commit(self: Box<Self>, source: &str) -> Result<std::path::PathBuf, DownloadError> {
        let path = self.base.commit(source)?;
        if self.at == At::Commit {
            (self.stop)();
        }
        Ok(path)
    }
}

fn setup(at: At, max_fetch: u64, fail: bool) -> (common::H, ToolCtx) {
    let mut h = common::harness();
    let c = common::ctx();
    let cancel = c.cancel.clone();
    let stop: Stop = Arc::new(move || cancel.cancel());
    h.tools = NetTools::new(NetToolsDeps {
        http: Arc::new(Http {
            at,
            stop: stop.clone(),
            fail,
        }),
        search: None,
        downloads: Arc::new(Store {
            base: h.downloads.clone(),
            at,
            stop,
        }),
        quarantine_root: None,
        broker: h.broker.clone(),
        deny: DenyLists::baseline(),
        env: PathEnv::windows_profile(&common::home()),
        config: NetToolsConfig {
            max_fetch_bytes: max_fetch,
            ..Default::default()
        },
        bus: Some(h.bus.clone()),
    });
    (h, c)
}

async fn call(h: &common::H, c: &ToolCtx, tool: &str, args: Value) -> ToolOutcome {
    h.tool(tool).call(args, c).await
}
fn cancelled(out: &ToolOutcome) {
    assert_eq!(out.status, ToolStatus::Cancelled, "{out:#?}");
    assert!(out.data.is_null());
    assert!(!out.text.contains("server-body"));
    assert!(out.text.contains("mogło już"), "{out:#?}");
    assert!(!out.text.contains("nie wykonano"));
}
fn no_commit(h: &common::H, aborted: usize) {
    assert!(
        h.downloads.files().is_empty(),
        "Po Stop nie ma zatwierdzonego pobrania"
    );
    assert_eq!(h.downloads.open_sinks(), 0);
    assert_eq!(h.downloads.aborted(), aborted);
}

#[tokio::test]
async fn head_stopped_at_headers_is_not_success() {
    let (h, c) = setup(At::Headers, 2048, false);
    cancelled(&call(&h, &c, "net_fetch", json!({"url":URL,"method":"head"})).await);
}
#[tokio::test]
async fn get_stopped_at_eof_does_not_publish_body() {
    let (h, c) = setup(At::Eof, 2048, false);
    cancelled(&call(&h, &c, "net_fetch", json!({"url":URL})).await);
}
#[tokio::test]
async fn download_stopped_at_eof_aborts_partial() {
    let (h, c) = setup(At::Eof, 2048, false);
    cancelled(&call(&h, &c, "net_download", json!({"url":URL})).await);
    no_commit(&h, 1);
}
#[tokio::test]
async fn cancelled_chunk_cannot_be_reported_as_size_truncation() {
    let (h, c) = setup(At::Chunk, 1, false);
    cancelled(&call(&h, &c, "net_fetch", json!({"url":URL})).await);
}
#[tokio::test]
async fn download_stopped_at_headers_never_begins_sink() {
    let (h, c) = setup(At::Headers, 2048, false);
    cancelled(&call(&h, &c, "net_download", json!({"url":URL})).await);
    no_commit(&h, 0);
}
#[tokio::test]
async fn header_error_after_stop_is_cancelled_for_head_and_download() {
    for tool in ["net_fetch", "net_download"] {
        let (h, c) = setup(At::Headers, 2048, true);
        let args = if tool == "net_fetch" {
            json!({"url":URL,"method":"head"})
        } else {
            json!({"url":URL})
        };
        cancelled(&call(&h, &c, tool, args).await);
        no_commit(&h, 0);
    }
}
#[tokio::test]
async fn stop_during_final_flush_prevents_commit() {
    let (h, c) = setup(At::Flush, 2048, false);
    cancelled(&call(&h, &c, "net_download", json!({"url":URL})).await);
    no_commit(&h, 1);
}
#[tokio::test]
async fn completed_commit_keeps_receipt_even_if_stop_arrives_after_it() {
    let (h, c) = setup(At::Commit, 2048, false);
    let out = call(&h, &c, "net_download", json!({"url":URL})).await;
    assert!(out.is_ok(), "{out:#?}");
    assert!(c.cancel.is_cancelled());
    assert_eq!(h.downloads.files().len(), 1);
    assert_eq!(h.downloads.open_sinks(), 0);
    assert_eq!(h.downloads.aborted(), 0);
    assert_eq!(out.data["bytes"], BODY.len());
}
#[tokio::test]
async fn ordinary_get_head_download_and_errors_keep_original_results() {
    for (tool, method, fail) in [
        ("net_fetch", "get", false),
        ("net_fetch", "head", false),
        ("net_download", "get", false),
        ("net_fetch", "head", true),
    ] {
        let (h, c) = setup(At::Never, 2048, fail);
        let args = if tool == "net_fetch" {
            json!({"url":URL,"method":method})
        } else {
            json!({"url":URL})
        };
        let out = call(&h, &c, tool, args).await;
        if fail {
            assert!(matches!(out.status, ToolStatus::Failed { .. }));
            assert!(out.text.contains("test-connect"));
        } else {
            assert!(out.is_ok(), "{out:#?}");
            if tool == "net_download" {
                assert_eq!(h.downloads.files().len(), 1);
                assert_eq!(h.downloads.files().values().next().unwrap().bytes, BODY);
            } else if method == "head" {
                assert_eq!(out.data["bytes"], 0);
            } else {
                assert_eq!(out.data["text"], "server-body");
            }
        }
    }
}
