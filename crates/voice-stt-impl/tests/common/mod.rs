//! Udawany `whisper-server` (HTTP/1.1 na 127.0.0.1) i launcher do testów: `/health` (najpierw 503),
//! `/inference` (odpowiedź `verbose_json` albo „awaria GPU” — zerwane połączenie i wyjście procesu).

#![allow(clippy::unwrap_used, clippy::expect_used, dead_code)]

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use device_profile_contract::Backend;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};
use voice_stt_contract::SttError;
use voice_stt_impl::sidecar::ExitInfo;
use voice_stt_impl::{LaunchSpec, Sidecar, SidecarLauncher};

pub const RESPONSE: &str = r#"{"task":"transcribe","language":"polish","duration":2.5,"text":" Delta, otwórz plik.","segments":[{"id":0,"text":" Delta, otwórz plik.","start":0.0,"end":2.0,"words":[{"word":" Delta,","start":0.1,"end":0.5,"probability":0.92},{"word":" otwórz","start":0.6,"end":1.0,"probability":0.88},{"word":" plik.","start":1.1,"end":1.6,"probability":0.97}],"avg_logprob":-0.1,"no_speech_prob":0.01}]}"#;

#[derive(Default)]
pub struct Shared {
    pub requests: Mutex<Vec<String>>,
    pub health_polls: AtomicUsize,
}

async fn read_request(stream: &mut BufReader<TcpStream>) -> Option<(String, Vec<u8>)> {
    let mut head = String::new();
    loop {
        let mut line = String::new();
        if stream.read_line(&mut line).await.ok()? == 0 {
            return None;
        }
        if line == "\r\n" {
            break;
        }
        head.push_str(&line);
    }
    let lower = head.to_lowercase();
    let mut body = Vec::new();
    if let Some(len) = lower
        .lines()
        .find_map(|l| l.strip_prefix("content-length:"))
    {
        let n: usize = len.trim().parse().ok()?;
        body.resize(n, 0);
        stream.read_exact(&mut body).await.ok()?;
    } else if lower.contains("transfer-encoding: chunked") {
        loop {
            let mut size = String::new();
            stream.read_line(&mut size).await.ok()?;
            let n = usize::from_str_radix(size.trim(), 16).ok()?;
            let mut chunk = vec![0; n + 2];
            stream.read_exact(&mut chunk).await.ok()?;
            if n == 0 {
                break;
            }
            body.extend_from_slice(&chunk[..n]);
        }
    }
    Some((head.lines().next()?.to_owned(), body))
}

async fn respond(stream: &mut BufReader<TcpStream>, code: u16, body: &str) {
    let msg = format!(
        "HTTP/1.1 {code} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    let _ = stream.get_mut().write_all(msg.as_bytes()).await;
    let _ = stream.get_mut().shutdown().await;
}

pub async fn serve(
    listener: TcpListener,
    shared: Arc<Shared>,
    exited: Arc<AtomicBool>,
    crash: bool,
    health_503: usize,
) {
    loop {
        let Ok((sock, _)) = listener.accept().await else {
            return;
        };
        let (shared, exited) = (shared.clone(), exited.clone());
        tokio::spawn(async move {
            let mut s = BufReader::new(sock);
            let Some((line, body)) = read_request(&mut s).await else {
                return;
            };
            if exited.load(Ordering::SeqCst) {
                return;
            }
            if line.starts_with("GET /health") {
                let n = shared.health_polls.fetch_add(1, Ordering::SeqCst);
                if n < health_503 {
                    respond(&mut s, 503, r#"{"status":"loading model"}"#).await;
                } else {
                    respond(&mut s, 200, r#"{"status":"ok"}"#).await;
                }
            } else if line.starts_with("POST /inference") {
                if crash {
                    // „ErrorDeviceLost”: proces kończy się w trakcie żądania.
                    exited.store(true, Ordering::SeqCst);
                    return;
                }
                shared
                    .requests
                    .lock()
                    .unwrap()
                    .push(String::from_utf8_lossy(&body).into_owned());
                respond(&mut s, 200, RESPONSE).await;
            } else {
                respond(&mut s, 404, "{}").await;
            }
        });
    }
}

pub struct FakeSidecar {
    port: u16,
    exited: Arc<AtomicBool>,
    task: tokio::task::JoinHandle<()>,
}

impl Sidecar for FakeSidecar {
    fn base_url(&self) -> String {
        format!("http://127.0.0.1:{}", self.port)
    }
    fn exited(&self) -> Option<ExitInfo> {
        self.exited.load(Ordering::SeqCst).then_some(ExitInfo {
            code: Some(-1),
            device_lost: true,
        })
    }
    fn kill(&self) {
        self.exited.store(true, Ordering::SeqCst);
        self.task.abort();
    }
}

#[derive(Default)]
pub struct FakeLauncher {
    pub crash: Vec<Backend>,
    pub health_503: usize,
    pub launches: Mutex<Vec<LaunchSpec>>,
    pub shared: Arc<Shared>,
}

#[async_trait]
impl SidecarLauncher for FakeLauncher {
    async fn launch(&self, spec: &LaunchSpec) -> Result<Box<dyn Sidecar>, SttError> {
        self.launches.lock().unwrap().push(spec.clone());
        let listener = TcpListener::bind(("127.0.0.1", spec.port))
            .await
            .map_err(|e| SttError::Sidecar(e.to_string()))?;
        let exited = Arc::new(AtomicBool::new(false));
        let task = tokio::spawn(serve(
            listener,
            self.shared.clone(),
            exited.clone(),
            self.crash.contains(&spec.backend),
            self.health_503,
        ));
        Ok(Box::new(FakeSidecar {
            port: spec.port,
            exited,
            task,
        }))
    }
}
