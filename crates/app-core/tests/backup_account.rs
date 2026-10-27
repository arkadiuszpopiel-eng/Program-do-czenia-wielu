//! Konto zapasowe (plan v6, zadanie C2.15): dwa konta tego samego dostawcy; gdy pierwsze
//! odpowiada limitem zapytań albo odrzuca klucz, kolejna wiadomość idzie przez drugie. Cała
//! ścieżka: prawdziwy adapter i Router do dwóch lokalnych serwerów udających dostawcę.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::Arc;
use std::sync::atomic::{AtomicU8, AtomicUsize, Ordering};

use app_core::AppCore;
use app_core::dto::{AccountState, AlfaEvent, SessionTemplate, ToastKind, TurnErrorCode};
use common::keys::{core, input, listed};
use common::{ends, send, until};

const KEY_A: &str = "test-pierwsze-XXXXXXXXXXXXXXXXX";
const KEY_B: &str = "test-zapasowe-XXXXXXXXXXXXXXXXX";

const WORKS: u8 = 0;
const LIMITED: u8 = 1;
const REVOKED: u8 = 2;

/// Dostawca zgodny z OpenAI z przełącznikiem: działa / limit zapytań w czacie / klucz odrzucony.
struct Stub {
    base: String,
    mode: Arc<AtomicU8>,
    chats: Arc<AtomicUsize>,
}

fn respond(stream: &mut std::net::TcpStream, status: &str, kind: &str, extra: &str, body: &str) {
    let response = format!(
        "HTTP/1.1 {status}\r\ncontent-type: {kind}\r\n{extra}content-length: {}\r\nconnection: close\r\n\r\n{body}",
        body.len()
    );
    let _ = stream.write_all(response.as_bytes());
}

fn stub(name: &'static str) -> Stub {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}/v1", listener.local_addr().unwrap());
    let mode = Arc::new(AtomicU8::new(WORKS));
    let chats = Arc::new(AtomicUsize::new(0));
    let (state, count) = (mode.clone(), chats.clone());
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };
            let mut buffer = Vec::new();
            let mut chunk = [0u8; 4096];
            let head_end = loop {
                if let Some(at) = buffer.windows(4).position(|w| w == b"\r\n\r\n") {
                    break Some(at + 4);
                }
                match stream.read(&mut chunk) {
                    Ok(0) | Err(_) => break None,
                    Ok(n) => buffer.extend_from_slice(&chunk[..n]),
                }
            };
            let Some(head_end) = head_end else { continue };
            let head = String::from_utf8_lossy(&buffer[..head_end]).to_string();
            let length = head
                .lines()
                .find_map(|line| {
                    let (name, value) = line.split_once(':')?;
                    name.eq_ignore_ascii_case("content-length")
                        .then(|| value.trim().parse::<usize>().ok())?
                })
                .unwrap_or(0);
            while buffer.len() < head_end + length {
                match stream.read(&mut chunk) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => buffer.extend_from_slice(&chunk[..n]),
                }
            }
            let json = "application/json";
            let mode = state.load(Ordering::SeqCst);
            if mode == REVOKED {
                let body = r#"{"error":{"message":"Incorrect API key provided.","type":"invalid_request_error","code":"invalid_api_key"}}"#;
                respond(&mut stream, "401 Unauthorized", json, "", body);
                continue;
            }
            if head.starts_with("GET") {
                let body = r#"{"object":"list","data":[{"id":"gpt-4o","object":"model"}]}"#;
                respond(&mut stream, "200 OK", json, "", body);
                continue;
            }
            count.fetch_add(1, Ordering::SeqCst);
            if mode == LIMITED {
                let body = r#"{"error":{"message":"Rate limit reached","type":"requests","code":"rate_limit_exceeded"}}"#;
                respond(
                    &mut stream,
                    "429 Too Many Requests",
                    json,
                    "retry-after: 120\r\n",
                    body,
                );
                continue;
            }
            let delta = format!(
                r#"{{"id":"x","object":"chat.completion.chunk","model":"gpt-4o","choices":[{{"index":0,"delta":{{"role":"assistant","content":"Odpowiada {name}."}},"finish_reason":null}}]}}"#
            );
            let done = r#"{"id":"x","object":"chat.completion.chunk","model":"gpt-4o","choices":[{"index":0,"delta":{},"finish_reason":"stop"}]}"#;
            let sse = format!("data: {delta}\n\ndata: {done}\n\ndata: [DONE]\n\n");
            respond(&mut stream, "200 OK", "text/event-stream", "", &sse);
        }
    });
    Stub { base, mode, chats }
}

struct Rig {
    core: AppCore,
    _dir: tempfile::TempDir,
    session: String,
    first: Stub,
    backup: Stub,
    first_id: String,
}

async fn rig() -> Rig {
    let (first, backup) = (stub("pierwsze"), stub("zapasowe"));
    let (core, dir) = core().await;
    let a = core
        .accounts_add(input(KEY_A, &first.base, "Pierwsze"))
        .await
        .unwrap();
    // Kolejność kont wyznacza data utworzenia — drugie ma być wyraźnie młodsze.
    tokio::time::sleep(std::time::Duration::from_millis(30)).await;
    let b = core
        .accounts_add(input(KEY_B, &backup.base, "Zapasowe"))
        .await
        .unwrap();
    assert!(core.accounts_test(a.id.clone()).await.unwrap().ok);
    assert!(core.accounts_test(b.id.clone()).await.unwrap().ok);
    let session = core
        .sessions_create(SessionTemplate::Empty)
        .await
        .unwrap()
        .id;
    Rig {
        core,
        _dir: dir,
        session,
        first,
        backup,
        first_id: a.id,
    }
}

impl Rig {
    /// Wysyła wiadomość i czeka na koniec tury; zwraca zdarzenia i tekst odpowiedzi.
    async fn ask(
        &self,
        rx: &mut tokio::sync::broadcast::Receiver<app_core::EventBatch>,
        text: &str,
    ) -> (Vec<AlfaEvent>, String) {
        let turn = self
            .core
            .turns_send(self.session.clone(), send(text, None))
            .await
            .unwrap()
            .assistant_turn_id
            .unwrap();
        let events = until(rx, ends(&turn)).await;
        let snap = self.core.turns_list(self.session.clone()).await.unwrap();
        let answer = snap
            .turns
            .iter()
            .find(|t| t.id == turn)
            .map(|t| t.text.clone())
            .unwrap_or_default();
        (events, answer)
    }

    fn chats(&self) -> (usize, usize) {
        (
            self.first.chats.load(Ordering::SeqCst),
            self.backup.chats.load(Ordering::SeqCst),
        )
    }
}

fn error_code(events: &[AlfaEvent]) -> Option<TurnErrorCode> {
    events.iter().find_map(|e| match e {
        AlfaEvent::Error { error, .. } => Some(error.code),
        _ => None,
    })
}

/// Czeka na komunikat o przełączeniu na konto zapasowe.
async fn switch_notice(
    rx: &mut tokio::sync::broadcast::Receiver<app_core::EventBatch>,
    seen: &[AlfaEvent],
) -> String {
    let is_notice = |e: &AlfaEvent| matches!(e, AlfaEvent::Toast { kind: ToastKind::Info, message } if message.pl.contains("konto zapasowe"));
    let text = |e: &AlfaEvent| match e {
        AlfaEvent::Toast { message, .. } => message.pl.clone(),
        _ => String::new(),
    };
    if let Some(found) = seen.iter().find(|e| is_notice(e)) {
        return text(found);
    }
    let later = until(rx, is_notice).await;
    later.iter().find(|e| is_notice(e)).map(text).unwrap()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn rate_limited_account_hands_over_to_the_backup() {
    let r = rig().await;
    let mut rx = r.core.subscribe_events();

    // Zwykła praca: odpowiada najstarsze konto, zapasowe nie jest ruszane.
    let (_, answer) = r.ask(&mut rx, "Pierwsze pytanie").await;
    assert_eq!(answer, "Odpowiada pierwsze.");
    assert_eq!(r.chats(), (1, 0));

    // Limit zapytań na pierwszym koncie: Router kończy turę na zapasowym, jest komunikat.
    r.first.mode.store(LIMITED, Ordering::SeqCst);
    let (events, answer) = r.ask(&mut rx, "Drugie pytanie").await;
    assert_eq!(error_code(&events), None);
    assert_eq!(answer, "Odpowiada zapasowe.");
    let notice = switch_notice(&mut rx, &events).await;
    assert!(
        notice.contains("Pierwsze") && notice.contains("Zapasowe"),
        "{notice}"
    );
    assert!(!notice.contains(KEY_A) && !notice.contains(KEY_B));

    // Następna wiadomość idzie przez konto zapasowe; pierwsze nie dostaje kolejnego zapytania.
    let asked_first = r.chats().0;
    let (events, answer) = r.ask(&mut rx, "Trzecie pytanie").await;
    assert_eq!(error_code(&events), None);
    assert_eq!(answer, "Odpowiada zapasowe.");
    assert_eq!(r.chats().0, asked_first);
    // Odstawione konto nie zmienia stanu — limit to nie błąd klucza.
    assert_eq!(listed(&r.core, &r.first_id).await.state, AccountState::Ok);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn rejected_key_is_rechecked_and_the_backup_takes_over() {
    let r = rig().await;
    let mut rx = r.core.subscribe_events();
    r.first.mode.store(REVOKED, Ordering::SeqCst);

    let (events, answer) = r.ask(&mut rx, "Pytanie").await;
    assert_eq!(error_code(&events), None);
    assert_eq!(answer, "Odpowiada zapasowe.");
    let notice = switch_notice(&mut rx, &events).await;
    assert!(
        notice.contains("Pierwsze") && notice.contains("Zapasowe"),
        "{notice}"
    );
    // Sprawdzenie w tle oznaczyło konto: odrzucony klucz widać na liście kont.
    let first = listed(&r.core, &r.first_id).await;
    assert_eq!(first.state, AccountState::Invalid);

    let (events, answer) = r.ask(&mut rx, "Drugie pytanie").await;
    assert_eq!(error_code(&events), None);
    assert_eq!(answer, "Odpowiada zapasowe.");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn single_account_is_never_benched() {
    let first = stub("jedyne");
    let (core, _dir) = core().await;
    let a = core
        .accounts_add(input(KEY_A, &first.base, "Jedyne"))
        .await
        .unwrap();
    assert!(core.accounts_test(a.id.clone()).await.unwrap().ok);
    let session = core
        .sessions_create(SessionTemplate::Empty)
        .await
        .unwrap()
        .id;
    let mut rx = core.subscribe_events();
    first.mode.store(LIMITED, Ordering::SeqCst);
    let turn = core
        .turns_send(session.clone(), send("Pytanie", None))
        .await
        .unwrap()
        .assistant_turn_id
        .unwrap();
    let events = until(&mut rx, ends(&turn)).await;
    assert_eq!(error_code(&events), Some(TurnErrorCode::RateLimited));
    tokio::time::sleep(std::time::Duration::from_millis(150)).await;
    let mut notices = 0;
    while let Ok(batch) = rx.try_recv() {
        notices += batch
            .iter()
            .filter(|e| matches!(e, AlfaEvent::Toast { message, .. } if message.pl.contains("zapasowe")))
            .count();
    }
    assert_eq!(notices, 0, "bez drugiego konta nie ma przełączenia");

    // Konto zostaje używalne — limit zapytań to nie błąd klucza.
    assert_eq!(listed(&core, &a.id).await.state, AccountState::Ok);
}
