//! Lokalny serwer fixture HTTP/1.1 (bez internetu): odtwarza zaprogramowane odpowiedzi
//! (także strumieniowo z pauzami), nagrywa żądania i moment rozłączenia klienta.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::time::Instant;

/// Fragment odpowiedzi.
#[derive(Debug, Clone)]
pub enum Part {
    /// Bajty do wysłania.
    Bytes(Vec<u8>),
    /// Pauza.
    Sleep(Duration),
    /// Zawieś się do rozłączenia klienta.
    Hang,
}

/// Zaprogramowana odpowiedź.
#[derive(Debug, Clone)]
pub struct Reply {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub parts: Vec<Part>,
}

impl Reply {
    pub fn sse(body: impl Into<String>) -> Self {
        Self {
            status: 200,
            headers: vec![("content-type".into(), "text/event-stream".into())],
            parts: vec![Part::Bytes(body.into().into_bytes())],
        }
    }

    pub fn sse_parts(parts: Vec<Part>) -> Self {
        Self {
            status: 200,
            headers: vec![("content-type".into(), "text/event-stream".into())],
            parts,
        }
    }

    pub fn json(status: u16, body: &serde_json::Value) -> Self {
        Self {
            status,
            headers: vec![("content-type".into(), "application/json".into())],
            parts: vec![Part::Bytes(body.to_string().into_bytes())],
        }
    }

    pub fn with_header(mut self, name: &str, value: &str) -> Self {
        self.headers.push((name.into(), value.into()));
        self
    }
}

/// Nagrane żądanie.
#[derive(Debug, Clone)]
pub struct Recorded {
    pub method: String,
    pub path: String,
    pub headers: Vec<(String, String)>,
    pub body: String,
}

impl Recorded {
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }

    pub fn json(&self) -> serde_json::Value {
        serde_json::from_str(&self.body).unwrap_or(serde_json::Value::Null)
    }
}

#[derive(Default)]
struct State {
    queue: VecDeque<Reply>,
    default: Option<Reply>,
    requests: Vec<Recorded>,
    disconnects: Vec<Instant>,
}

/// Serwer fixture.
#[derive(Clone)]
pub struct FixtureServer {
    url: String,
    state: Arc<Mutex<State>>,
}

fn lock(m: &Mutex<State>) -> MutexGuard<'_, State> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

impl FixtureServer {
    pub async fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let state: Arc<Mutex<State>> = Arc::default();
        let st = Arc::clone(&state);
        tokio::spawn(async move {
            while let Ok((sock, _)) = listener.accept().await {
                tokio::spawn(handle(sock, Arc::clone(&st)));
            }
        });
        Self { url, state }
    }

    pub fn url(&self) -> String {
        self.url.clone()
    }

    /// Odpowiedź domyślna (gdy kolejka pusta) + wyczyszczenie historii.
    pub fn reset(&self, default: Reply) {
        let mut st = lock(&self.state);
        *st = State {
            default: Some(default),
            ..State::default()
        };
    }

    pub fn push(&self, reply: Reply) {
        lock(&self.state).queue.push_back(reply);
    }

    pub fn requests(&self) -> Vec<Recorded> {
        lock(&self.state).requests.clone()
    }

    pub fn disconnects(&self) -> Vec<Instant> {
        lock(&self.state).disconnects.clone()
    }
}

async fn read_request(sock: &mut TcpStream) -> Option<Recorded> {
    let mut buf = Vec::new();
    let mut chunk = [0u8; 8192];
    let header_end = loop {
        let n = sock.read(&mut chunk).await.ok()?;
        if n == 0 {
            return None;
        }
        buf.extend_from_slice(&chunk[..n]);
        if let Some(p) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
            break p + 4;
        }
    };
    let head = String::from_utf8_lossy(&buf[..header_end]).to_string();
    let mut lines = head.split("\r\n");
    let mut first = lines.next()?.split(' ');
    let method = first.next()?.to_owned();
    let path = first.next()?.to_owned();
    let headers: Vec<(String, String)> = lines
        .filter_map(|l| l.split_once(':'))
        .map(|(k, v)| (k.trim().to_ascii_lowercase(), v.trim().to_owned()))
        .collect();
    let len = headers
        .iter()
        .find(|(k, _)| k == "content-length")
        .and_then(|(_, v)| v.parse::<usize>().ok())
        .unwrap_or(0);
    let mut body = buf[header_end..].to_vec();
    while body.len() < len {
        let n = sock.read(&mut chunk).await.ok()?;
        if n == 0 {
            break;
        }
        body.extend_from_slice(&chunk[..n]);
    }
    Some(Recorded {
        method,
        path,
        headers,
        body: String::from_utf8_lossy(&body).to_string(),
    })
}

async fn handle(mut sock: TcpStream, state: Arc<Mutex<State>>) {
    let Some(req) = read_request(&mut sock).await else {
        return;
    };
    let reply = {
        let mut st = lock(&state);
        st.requests.push(req);
        st.queue.pop_front().or_else(|| st.default.clone())
    };
    let Some(reply) = reply else { return };
    let mut head = format!("HTTP/1.1 {} X\r\nconnection: close\r\n", reply.status);
    for (k, v) in &reply.headers {
        head.push_str(&format!("{k}: {v}\r\n"));
    }
    head.push_str("\r\n");
    let (mut rd, mut wr) = sock.into_split();
    if wr.write_all(head.as_bytes()).await.is_err() {
        return;
    }
    // Wykrywanie rozłączenia klienta (FIN/RST) równolegle z wysyłaniem.
    let watcher_state = Arc::clone(&state);
    let watcher = tokio::spawn(async move {
        let mut b = [0u8; 64];
        loop {
            match rd.read(&mut b).await {
                Ok(0) | Err(_) => {
                    lock(&watcher_state).disconnects.push(Instant::now());
                    return;
                }
                Ok(_) => {}
            }
        }
    });
    for part in reply.parts {
        match part {
            Part::Bytes(b) => {
                if wr.write_all(&b).await.is_err() || wr.flush().await.is_err() {
                    return;
                }
            }
            Part::Sleep(d) => tokio::time::sleep(d).await,
            Part::Hang => {
                let _ = watcher.await;
                return;
            }
        }
    }
    let _ = wr.shutdown().await;
    watcher.abort();
}
