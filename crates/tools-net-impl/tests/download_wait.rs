//! Kontrolowane blokowanie magazynu: Stop/deadline bez odłączania rozpoczętego commit.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use async_trait::async_trait;
use compliance_contract::{DenyLists, PathEnv};
use platform_apps_contract::{DownloadError, DownloadSink, DownloadStore};
use platform_apps_fake::FakeDownloads;
use serde_json::json;
use std::future::{Future, poll_fn};
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::{Arc, Condvar, Mutex};
use std::task::Poll;
use std::time::Duration;
use tokio::sync::Notify;
use tools_common_contract::{ToolCtx, ToolErrorKind, ToolOutcome, ToolStatus};
use tools_net_contract::{
    BodyReader, HttpPort, HttpRequest, HttpResponse, NetError, NetToolsConfig,
};
use tools_net_impl::{NetTools, NetToolsDeps};

#[derive(Clone, Copy, PartialEq)]
enum Phase {
    Begin,
    Write,
    Commit,
}

#[derive(Default)]
struct Gate {
    entered: Notify,
    finished: Notify,
    released: Mutex<bool>,
    condition: Condvar,
}
impl Gate {
    fn block(&self) {
        let mut release = self.released.lock().unwrap();
        self.entered.notify_one();
        while !*release {
            release = self.condition.wait(release).unwrap();
        }
    }
    fn release(&self) {
        *self.released.lock().unwrap() = true;
        self.condition.notify_all();
    }
}
struct Release(Arc<Gate>);
impl Drop for Release {
    fn drop(&mut self) {
        self.0.release();
    }
}

struct Store {
    base: FakeDownloads,
    phase: Phase,
    gate: Arc<Gate>,
}
struct Sink {
    base: Option<Box<dyn DownloadSink>>,
    phase: Phase,
    gate: Arc<Gate>,
}
impl DownloadStore for Store {
    fn begin(&self, dir: &Path, name: &str) -> Result<Box<dyn DownloadSink>, DownloadError> {
        let sink = Sink {
            base: Some(self.base.begin(dir, name)?),
            phase: self.phase,
            gate: self.gate.clone(),
        };
        if self.phase == Phase::Begin {
            self.gate.block();
        }
        Ok(Box::new(sink))
    }
}
impl DownloadSink for Sink {
    fn write(&mut self, bytes: &[u8]) -> Result<(), DownloadError> {
        self.base.as_mut().unwrap().write(bytes)?;
        if self.phase == Phase::Write {
            self.gate.block();
        }
        Ok(())
    }
    fn commit(mut self: Box<Self>, source: &str) -> Result<PathBuf, DownloadError> {
        if self.phase == Phase::Commit {
            self.gate.block();
        }
        self.base.take().unwrap().commit(source)
    }
}
impl Drop for Sink {
    fn drop(&mut self) {
        drop(self.base.take());
        self.gate.finished.notify_one();
    }
}

struct Http;
struct Body(bool);
#[async_trait]
impl HttpPort for Http {
    async fn send(&self, _: &HttpRequest) -> Result<HttpResponse, NetError> {
        Ok(HttpResponse {
            status: 200,
            content_type: Some("text/plain".into()),
            content_length: None,
            location: None,
            content_disposition: None,
            body: Box::new(Body(false)),
        })
    }
}
#[async_trait]
impl BodyReader for Body {
    async fn chunk(&mut self) -> Result<Option<Vec<u8>>, NetError> {
        if self.0 {
            Ok(None)
        } else {
            self.0 = true;
            Ok(Some(b"data".to_vec()))
        }
    }
}

struct Fixture {
    h: common::H,
    ctx: ToolCtx,
    gate: Arc<Gate>,
    _release: Release,
}
fn fixture(phase: Phase) -> Fixture {
    let mut h = common::harness();
    let ctx = common::ctx();
    let gate = Arc::new(Gate::default());
    h.tools = NetTools::new(NetToolsDeps {
        http: Arc::new(Http),
        search: None,
        downloads: Arc::new(Store {
            base: h.downloads.clone(),
            phase,
            gate: gate.clone(),
        }),
        quarantine_root: None,
        broker: h.broker.clone(),
        deny: DenyLists::baseline(),
        env: PathEnv::windows_profile(&common::home()),
        config: NetToolsConfig::default(),
        bus: Some(h.bus.clone()),
    });
    Fixture {
        h,
        ctx,
        gate: gate.clone(),
        _release: Release(gate),
    }
}

async fn enter<F: Future<Output = ToolOutcome>>(call: &mut Pin<Box<F>>, gate: &Gate) {
    tokio::select! {
        out=call.as_mut()=>panic!("Przedwczesny wynik: {out:?}"),
        ()=gate.entered.notified()=>{},
        ()=tokio::time::sleep(Duration::from_secs(5))=>panic!("Watchdog rozpoczęcia I/O"),
    }
}
async fn poll_once<F: Future>(call: &mut Pin<Box<F>>) -> Poll<F::Output> {
    poll_fn(|cx| Poll::Ready(call.as_mut().poll(cx))).await
}
async fn cleanup(f: &Fixture, committed: bool) {
    f.gate.release();
    tokio::time::timeout(Duration::from_secs(5), f.gate.finished.notified())
        .await
        .expect("Pracownik kończy po zwolnieniu bramki");
    assert_eq!(f.h.downloads.open_sinks(), 0);
    assert_eq!(f.h.downloads.files().len(), usize::from(committed));
    assert_eq!(f.h.downloads.aborted(), usize::from(!committed));
}

async fn interrupted(phase: Phase, deadline: bool) {
    let f = fixture(phase);
    let tool = f.h.tool("net_download");
    let mut call = Box::pin(tool.call(json!({"url":"https://files.example/a.txt"}), &f.ctx));
    enter(&mut call, &f.gate).await;
    if deadline {
        tokio::time::pause();
        tokio::time::advance(Duration::from_millis(
            NetToolsConfig::default().download_timeout_ms + 1,
        ))
        .await;
    } else {
        f.ctx.cancel.cancel();
    }
    let result = poll_once(&mut call).await;
    if deadline {
        tokio::time::resume();
    }
    let Poll::Ready(out) = result else {
        panic!("Oczekiwanie na magazyn ignoruje Stop/deadline")
    };
    if deadline {
        assert_eq!(
            out.status,
            ToolStatus::Failed {
                error: ToolErrorKind::Timeout
            }
        );
    } else {
        assert_eq!(out.status, ToolStatus::Cancelled);
    }
    assert!(out.data.is_null());
    assert!(out.text.contains("w tle"), "{}", out.text);
    assert!(f.h.downloads.files().is_empty());
    assert_eq!(
        f.h.downloads.open_sinks(),
        1,
        "I/O wciąż trwa; nie udajemy natychmiastowego sprzątnięcia"
    );
    cleanup(&f, false).await;
}

async fn commit_is_observed(deadline: bool) {
    let f = fixture(Phase::Commit);
    let tool = f.h.tool("net_download");
    let mut call = Box::pin(tool.call(json!({"url":"https://files.example/a.txt"}), &f.ctx));
    enter(&mut call, &f.gate).await;
    if deadline {
        tokio::time::pause();
        tokio::time::advance(Duration::from_millis(
            NetToolsConfig::default().download_timeout_ms + 1,
        ))
        .await;
    } else {
        f.ctx.cancel.cancel();
    }
    let result = poll_once(&mut call).await;
    if deadline {
        tokio::time::resume();
    }
    assert!(
        result.is_pending(),
        "Nie odłączamy rozpoczętego commit bez jego rzeczywistego wyniku"
    );
    f.gate.release();
    let out = call.await;
    assert!(out.is_ok(), "{out:?}");
    cleanup(&f, true).await;
}

#[tokio::test]
async fn stop_during_begin_stops_waiting_before_worker_returns() {
    interrupted(Phase::Begin, false).await;
}
#[tokio::test]
async fn stop_during_write_stops_waiting_before_worker_returns() {
    interrupted(Phase::Write, false).await;
}
#[tokio::test]
async fn existing_deadline_bounds_wait_for_begin() {
    interrupted(Phase::Begin, true).await;
}
#[tokio::test]
async fn existing_deadline_bounds_wait_for_write() {
    interrupted(Phase::Write, true).await;
}
#[tokio::test]
async fn stop_does_not_hide_result_of_started_commit() {
    commit_is_observed(false).await;
}
#[tokio::test]
async fn deadline_does_not_hide_result_of_started_commit() {
    commit_is_observed(true).await;
}
