//! Lokalny serwer udający dostawcę zgodnego z OpenAI — lista modeli zmieszana jak u prawdziwego
//! dostawcy (synteza mowy, transkrypcja, embeddingi, rozmowa) i strumieniowa odpowiedź czatu.
//! Zapisuje, o który model poproszono, żeby testy widziały decyzje Routera od strony sieci.

use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};

/// Modele w kolejności, w jakiej zwraca je dostawca (pierwszy NIE jest modelem rozmowy).
pub const LISTED: [&str; 7] = [
    "tts-1",
    "whisper-1",
    "text-embedding-3-small",
    "dall-e-3",
    "gpt-4o-mini",
    "gpt-4o",
    "gpt-4o-2024-08-06",
];

/// Model, którego konto „nie ma” — serwer odpowiada jak OpenAI na nieistniejący model.
pub const MISSING: &str = "gpt-wycofany";

fn models_body() -> String {
    let data: Vec<String> = LISTED
        .iter()
        .map(|id| format!(r#"{{"id":"{id}","object":"model"}}"#))
        .collect();
    format!(r#"{{"object":"list","data":[{}]}}"#, data.join(","))
}

fn requested_model(body: &str) -> String {
    serde_json::from_str::<serde_json::Value>(body)
        .ok()
        .and_then(|v| v["model"].as_str().map(str::to_owned))
        .unwrap_or_default()
}

fn respond(stream: &mut std::net::TcpStream, status: &str, kind: &str, body: &str) {
    let response = format!(
        "HTTP/1.1 {status}\r\ncontent-type: {kind}\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
        body.len()
    );
    let _ = stream.write_all(response.as_bytes());
}

/// Uruchamia serwer; zwraca adres bazowy i listę modeli, o które poproszono w czacie.
pub fn chat_stub() -> (String, Arc<Mutex<Vec<String>>>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}/v1", listener.local_addr().unwrap());
    let asked = Arc::new(Mutex::new(Vec::new()));
    let log = asked.clone();
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
            let body = String::from_utf8_lossy(&buffer[head_end..]).to_string();
            if head.starts_with("GET") {
                respond(&mut stream, "200 OK", "application/json", &models_body());
                continue;
            }
            let model = requested_model(&body);
            log.lock().unwrap().push(model.clone());
            if model == MISSING || model == "tts-1" {
                let error = format!(
                    r#"{{"error":{{"message":"The model `{model}` does not exist or you do not have access to it.","type":"invalid_request_error","code":"model_not_found"}}}}"#
                );
                respond(&mut stream, "404 Not Found", "application/json", &error);
                continue;
            }
            let delta = format!(
                r#"{{"id":"x","object":"chat.completion.chunk","model":"{model}","choices":[{{"index":0,"delta":{{"role":"assistant","content":"Odpowiada {model}."}},"finish_reason":null}}]}}"#
            );
            let done = format!(
                r#"{{"id":"x","object":"chat.completion.chunk","model":"{model}","choices":[{{"index":0,"delta":{{}},"finish_reason":"stop"}}]}}"#
            );
            let sse = format!("data: {delta}\n\ndata: {done}\n\ndata: [DONE]\n\n");
            respond(&mut stream, "200 OK", "text/event-stream", &sse);
        }
    });
    (base, asked)
}
