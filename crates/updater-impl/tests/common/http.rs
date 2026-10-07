//! Minimalny serwer HTTP/1.1 na 127.0.0.1 dla testów pobierania: `GET` plików z pamięci,
//! `Range: bytes=<n>-` (206 / 416), opcjonalnie bez obsługi zakresów, jednorazowe przerwanie
//! odpowiedzi po N bajtach treści albo zawieszenie (test anulowania), dziennik żądań.

#![allow(dead_code, clippy::unwrap_used, clippy::expect_used)]

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

#[derive(Default)]
pub struct State {
    pub files: HashMap<String, Vec<u8>>,
    /// Następna odpowiedź z treścią: tylko tyle bajtów, potem zerwanie połączenia.
    pub cut_next_after: Option<usize>,
    /// Następna odpowiedź z treścią: tyle bajtów, potem zawieszenie (bez zamykania).
    pub stall_next_after: Option<usize>,
    /// Serwer ignoruje nagłówek Range (zawsze 200).
    pub no_range: bool,
    /// Żądania: (ścieżka, nagłówek Range).
    pub requests: Vec<(String, Option<String>)>,
}

#[derive(Clone)]
pub struct Server {
    pub base: String,
    pub state: Arc<Mutex<State>>,
}

impl Server {
    pub async fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let state = Arc::new(Mutex::new(State::default()));
        let shared = state.clone();
        tokio::spawn(async move {
            while let Ok((stream, _)) = listener.accept().await {
                tokio::spawn(serve(stream, shared.clone()));
            }
        });
        Self { base, state }
    }

    pub fn put(&self, path: &str, bytes: Vec<u8>) {
        self.state
            .lock()
            .unwrap()
            .files
            .insert(path.to_owned(), bytes);
    }

    pub fn with<T>(&self, f: impl FnOnce(&mut State) -> T) -> T {
        f(&mut self.state.lock().unwrap())
    }

    pub fn requests(&self) -> Vec<(String, Option<String>)> {
        self.state.lock().unwrap().requests.clone()
    }
}

async fn serve(mut stream: TcpStream, state: Arc<Mutex<State>>) {
    let mut head = Vec::new();
    let mut byte = [0u8; 1];
    while !head.ends_with(b"\r\n\r\n") {
        match stream.read(&mut byte).await {
            Ok(1) => head.push(byte[0]),
            _ => return,
        }
    }
    let text = String::from_utf8_lossy(&head).into_owned();
    let path = text
        .lines()
        .next()
        .and_then(|l| l.split_whitespace().nth(1))
        .unwrap_or("/")
        .to_owned();
    let range = text.lines().find_map(|l| {
        let (k, v) = l.split_once(':')?;
        k.eq_ignore_ascii_case("range").then(|| v.trim().to_owned())
    });
    let (file, no_range, cut, stall) = {
        let mut st = state.lock().unwrap();
        st.requests.push((path.clone(), range.clone()));
        let file = st.files.get(&path).cloned();
        let (cut, stall) = if file.is_some() {
            (st.cut_next_after.take(), st.stall_next_after.take())
        } else {
            (None, None)
        };
        (file, st.no_range, cut, stall)
    };
    let Some(file) = file else {
        let _ = stream
            .write_all(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
            .await;
        return;
    };
    let len = file.len();
    let start = range
        .as_deref()
        .and_then(|r| r.strip_prefix("bytes="))
        .and_then(|r| r.strip_suffix('-'))
        .and_then(|n| n.parse::<usize>().ok())
        .filter(|_| !no_range);
    let (status, body, extra) = match start {
        Some(s) if s >= len => (
            "416 Range Not Satisfiable",
            Vec::new(),
            format!("Content-Range: bytes */{len}\r\n"),
        ),
        Some(s) => (
            "206 Partial Content",
            file[s..].to_vec(),
            format!("Content-Range: bytes {s}-{}/{len}\r\n", len - 1),
        ),
        None => ("200 OK", file, String::new()),
    };
    let header = format!(
        "HTTP/1.1 {status}\r\nContent-Length: {}\r\n{extra}Connection: close\r\n\r\n",
        body.len()
    );
    if stream.write_all(header.as_bytes()).await.is_err() {
        return;
    }
    if let Some(n) = cut {
        let _ = stream.write_all(&body[..n.min(body.len())]).await;
        let _ = stream.flush().await;
        return;
    }
    if let Some(n) = stall {
        let _ = stream.write_all(&body[..n.min(body.len())]).await;
        let _ = stream.flush().await;
        tokio::time::sleep(Duration::from_secs(300)).await;
        return;
    }
    let _ = stream.write_all(&body).await;
    let _ = stream.flush().await;
}
