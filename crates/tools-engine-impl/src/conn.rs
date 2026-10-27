//! Połączenie JSON-RPC z serwerem silnika: jedno gniazdo, numerowane żądania, limit czasu.
//! Serwer zrywa połączenie po 120 s ciszy — przed wywołaniem po dłuższej przerwie łączymy się
//! od nowa (nigdy nie ponawiamy wysłanego żądania: krok i cofnięcie nie są idempotentne).

use std::time::{Duration, Instant};

use serde_json::Value;
use tokio::net::TcpStream;
use tools_engine_contract::{
    EngineError, EngineMethod, IDLE_RECONNECT_SECS, MAX_MESSAGE_BYTES, request, response,
};

use crate::ws::{WsError, WsStream};

/// Limit nawiązania połączenia.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);

fn transport(e: WsError) -> EngineError {
    match e {
        WsError::TooLarge(limit) => EngineError::TooLarge {
            bytes: limit + 1,
            limit,
        },
        other => EngineError::Transport(other.to_string()),
    }
}

/// Połączenie z jednym serwerem.
pub(crate) struct Connection {
    host: String,
    ws: Option<WsStream<TcpStream>>,
    next_id: u64,
    last_used: Instant,
}

impl Connection {
    /// Łączy się z `host:port` (tylko pętla zwrotna — sprawdzone w [`tools_engine_contract::Listening`]).
    pub(crate) async fn open(host: &str) -> Result<Self, EngineError> {
        let mut conn = Self {
            host: host.to_owned(),
            ws: None,
            next_id: 1,
            last_used: Instant::now(),
        };
        conn.reconnect().await?;
        Ok(conn)
    }

    async fn reconnect(&mut self) -> Result<(), EngineError> {
        if let Some(mut old) = self.ws.take() {
            old.close().await;
        }
        let tcp = tokio::time::timeout(CONNECT_TIMEOUT, TcpStream::connect(&self.host))
            .await
            .map_err(|_| EngineError::Timeout(CONNECT_TIMEOUT.as_secs()))?
            .map_err(|e| EngineError::Transport(e.to_string()))?;
        // Nagle wyłączony — małe żądania JSON bez opóźnienia.
        let _ = tcp.set_nodelay(true);
        let ws = tokio::time::timeout(
            CONNECT_TIMEOUT,
            WsStream::handshake(tcp, &self.host, "/", MAX_MESSAGE_BYTES),
        )
        .await
        .map_err(|_| EngineError::Timeout(CONNECT_TIMEOUT.as_secs()))?
        .map_err(transport)?;
        self.ws = Some(ws);
        self.last_used = Instant::now();
        Ok(())
    }

    /// Wywołuje metodę i czeka na odpowiedź z tym samym numerem (inne pomija — np. spóźnioną
    /// odpowiedź na żądanie, na które przestaliśmy czekać).
    pub(crate) async fn call(
        &mut self,
        method: EngineMethod,
        params: &Value,
        limit: Duration,
    ) -> Result<Value, EngineError> {
        let id = self.next_id;
        self.next_id += 1;
        let text = request(id, method, params)?;
        if self.ws.is_none() || self.last_used.elapsed().as_secs() >= IDLE_RECONNECT_SECS {
            self.reconnect().await?;
        }
        let ws = self
            .ws
            .as_mut()
            .ok_or_else(|| EngineError::Transport("brak połączenia".into()))?;
        let exchange = async {
            ws.send_text(&text).await.map_err(transport)?;
            loop {
                let reply = ws.recv_text().await.map_err(transport)?;
                if let Some(result) = response(id, &reply)? {
                    return Ok(result);
                }
            }
        };
        let result = match tokio::time::timeout(limit, exchange).await {
            Ok(r) => r,
            Err(_) => Err(EngineError::Timeout(limit.as_secs())),
        };
        self.last_used = Instant::now();
        // Po błędzie łącza gniazdo jest w nieznanym stanie — następne wywołanie połączy się od nowa.
        if matches!(
            result,
            Err(EngineError::Transport(_) | EngineError::Protocol(_))
        ) {
            self.ws = None;
        }
        result
    }

    /// Zamyka połączenie.
    pub(crate) async fn close(&mut self) {
        if let Some(mut ws) = self.ws.take() {
            ws.close().await;
        }
    }
}
