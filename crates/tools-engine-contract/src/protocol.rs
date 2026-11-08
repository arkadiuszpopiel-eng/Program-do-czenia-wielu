//! Protokół serwera silnika NextGO.Gray (`engine-server.exe`): pierwszy wiersz stdout, metody
//! JSON-RPC 2.0 i błędy. Źródło prawdy: `crates/02_mcp_director_server` w repozytorium silnika
//! (`src/rpc.rs`, `src/server.rs`, `src/bin/engine_server.rs`) — opis w SPEC modułu.

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::EngineError;

/// Największa wiadomość w obie strony (limit serwera: 1 MiB).
pub const MAX_MESSAGE_BYTES: usize = 1024 * 1024;
/// Serwer zrywa połączenie po 120 s bez wiadomości — po dłuższej ciszy klient łączy się od nowa.
pub const IDLE_RECONNECT_SECS: u64 = 90;

/// Pierwszy wiersz stdout serwera: `{"listening":"ws://127.0.0.1:PORT","mode":…,"paused":…}`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Listening {
    /// Adres WebSocket (port rzeczywisty, także po `--listen 127.0.0.1:0`).
    pub listening: String,
    /// `headless` albo `gpu_preview`.
    pub mode: String,
    /// Czas symulacji stoi (tylko wywołania go przesuwają).
    pub paused: bool,
}

impl Listening {
    /// Parsuje wiersz i sprawdza, że adres to pętla zwrotna (`127.0.0.1` / `[::1]`) z portem.
    pub fn parse(line: &str) -> Result<Self, EngineError> {
        let parsed: Self = serde_json::from_str(line.trim()).map_err(|e| {
            EngineError::Protocol(format!(
                "pierwszy wiersz serwera nie jest opisem adresu: {e}"
            ))
        })?;
        parsed.host_port()?;
        Ok(parsed)
    }

    /// `host:port` do połączenia TCP — tylko pętla zwrotna, bez ścieżki, zapytania i użytkownika.
    pub fn host_port(&self) -> Result<String, EngineError> {
        let rest = self
            .listening
            .strip_prefix("ws://")
            .ok_or_else(|| EngineError::Protocol("adres serwera bez ws://".into()))?;
        let rest = rest.strip_suffix('/').unwrap_or(rest);
        let (host, port) = rest
            .rsplit_once(':')
            .ok_or_else(|| EngineError::Protocol("adres serwera bez portu".into()))?;
        let loopback = host == "127.0.0.1" || host == "[::1]";
        let port_ok = port.parse::<u16>().is_ok_and(|p| p != 0);
        if !loopback || !port_ok || rest.contains(['/', '?', '@', '#']) {
            return Err(EngineError::Protocol(format!(
                "serwer silnika musi słuchać tylko lokalnie (dostał „{}”)",
                self.listening
            )));
        }
        Ok(rest.to_owned())
    }
}

/// Metody JSON-RPC serwera.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EngineMethod {
    /// Atomowa transakcja operacji (`snapshot` RAM, idempotentna po `transaction_id`).
    ExecuteTransaction,
    /// Cofa ostatnią zmianę (historia do 64 kroków).
    RollbackLastChange,
    /// Kroki symulacji `{frames 0..10000}`.
    HeadlessStep,
    /// Zaznaczenie encji `{entity_id|null}`.
    SetSpatialSelection,
    /// Odczyt zaznaczenia.
    GetSpatialSelection,
    /// Podmiana shadera WGSL (wewnętrznie transakcja `shader.set`).
    HotReloadShader,
    /// Telemetria sceny (encje w promieniu, renderer, fizyka, dźwięk, nawigacja).
    InspectSceneTelemetry,
    /// Stan strumieniowania terenu.
    InspectWorldStreaming,
    /// Przyjęcie gotowych fragmentów terenu `{budget 1..32}`.
    StreamWorld,
    /// Promień w świecie.
    RaycastWorld,
    /// Wysokość terenu `{x, z}`.
    QueryWorldHeight,
}

impl EngineMethod {
    /// Nazwa metody w protokole.
    pub fn wire(self) -> &'static str {
        match self {
            Self::ExecuteTransaction => "engine.execute_transaction",
            Self::RollbackLastChange => "engine.rollback_last_change",
            Self::HeadlessStep => "engine.headless_step",
            Self::SetSpatialSelection => "engine.set_spatial_selection",
            Self::GetSpatialSelection => "engine.get_spatial_selection",
            Self::HotReloadShader => "engine.hot_reload_shader",
            Self::InspectSceneTelemetry => "engine.inspect_scene_telemetry",
            Self::InspectWorldStreaming => "engine.inspect_world_streaming",
            Self::StreamWorld => "engine.stream_world",
            Self::RaycastWorld => "engine.raycast_world",
            Self::QueryWorldHeight => "engine.query_world_height",
        }
    }

    /// Czy metoda zmienia stan silnika (rewizję) — zgoda Brokera jak dla zmiany.
    pub fn mutating(self) -> bool {
        !matches!(
            self,
            Self::GetSpatialSelection
                | Self::InspectSceneTelemetry
                | Self::InspectWorldStreaming
                | Self::RaycastWorld
                | Self::QueryWorldHeight
        )
    }
}

/// Treść żądania (zawsze z `id` — żądanie bez `id` serwer wykonuje bez odpowiedzi).
pub fn request(id: u64, method: EngineMethod, params: &Value) -> Result<String, EngineError> {
    let text =
        json!({"jsonrpc": "2.0", "id": id, "method": method.wire(), "params": params}).to_string();
    if text.len() > MAX_MESSAGE_BYTES {
        return Err(EngineError::TooLarge {
            bytes: text.len(),
            limit: MAX_MESSAGE_BYTES,
        });
    }
    Ok(text)
}

/// Rozbiera odpowiedź na żądanie `id`: wynik albo błąd silnika. `Ok(None)` — wiadomość
/// z innym `id` (nie dotyczy tego żądania).
pub fn response(id: u64, text: &str) -> Result<Option<Value>, EngineError> {
    let v: Value = serde_json::from_str(text)
        .map_err(|e| EngineError::Protocol(format!("odpowiedź nie jest JSON: {e}")))?;
    if v.get("jsonrpc").and_then(Value::as_str) != Some("2.0") {
        return Err(EngineError::Protocol("odpowiedź bez jsonrpc 2.0".into()));
    }
    match v.get("id") {
        Some(Value::Number(n)) if n.as_u64() == Some(id) => {}
        // Błąd bez numeru (parse error, invalid request) dotyczy ostatniego żądania.
        Some(Value::Null) if v.get("error").is_some() => {}
        _ => return Ok(None),
    }
    if let Some(err) = v.get("error") {
        return Err(EngineError::Rpc {
            code: err.get("code").and_then(Value::as_i64).unwrap_or(0),
            kind: err
                .get("data")
                .and_then(|d| d.get("code"))
                .and_then(Value::as_str)
                .unwrap_or("unknown")
                .to_owned(),
            message: err
                .get("message")
                .and_then(Value::as_str)
                .unwrap_or("błąd bez opisu")
                .to_owned(),
        });
    }
    v.get("result")
        .cloned()
        .map(Some)
        .ok_or_else(|| EngineError::Protocol("odpowiedź bez result i error".into()))
}

/// Wskazówka dla modelu do kodu błędu silnika (`data.code`).
pub fn hint(kind: &str) -> &'static str {
    match kind {
        "streaming_not_ready" => {
            "Teren nie jest gotowy — wywołaj `engine_stream` (until_ready), potem powtórz."
        }
        "revision_conflict" => {
            "Rewizja się zmieniła — odczytaj stan (`engine_inspect`) i wyślij transakcję z nową `expected_revision`."
        }
        "duplicate_transaction" => {
            "Ten `transaction_id` był już użyty z inną treścią — nadaj nowy identyfikator."
        }
        "entity_not_found" => "Nie ma takiej encji — sprawdź identyfikatory w `engine_inspect`.",
        "no_snapshot" => "Historia zmian jest pusta — nie ma czego cofnąć.",
        "capacity_exceeded" => {
            "Przekroczony limit silnika — zmniejsz liczbę klatek/obiektów albo uruchom silnik od nowa."
        }
        "invalid_shader" => "Shader WGSL nie przeszedł walidacji — popraw kod według komunikatu.",
        "response_too_large" => "Odpowiedź za duża — zmniejsz `limit` albo promień.",
        "invalid_params" | "invalid_input" | "invalid_saved_state" => {
            "Popraw argumenty według komunikatu silnika i dokumentacji (`engine_docs`)."
        }
        _ => "Sprawdź komunikat silnika i dokumentację (`engine_docs`).",
    }
}
