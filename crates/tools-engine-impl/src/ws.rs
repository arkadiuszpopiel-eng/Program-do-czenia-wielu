//! Minimalny klient WebSocket (RFC 6455) tylko dla pętli zwrotnej: ramki tekstowe, ping/pong,
//! zamknięcie, łączenie fragmentów. Bez rozszerzeń, bez TLS i bez nagłówka `Origin` (serwer
//! silnika odrzuca połączenia z `Origin` — ochrona przed stronami WWW).

use base64::Engine as _;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

use crate::sha1::sha1;

const GUID: &str = "258EAFA5-E914-47DA-95CA-C5AB0DC85B11";
/// Najdłuższa odpowiedź nagłówków uzgadniania.
const MAX_HEADERS: usize = 16 * 1024;

/// Błąd połączenia.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum WsError {
    /// Wejście/wyjście.
    #[error("połączenie: {0}")]
    Io(String),
    /// Serwer nie przyjął uzgadniania.
    #[error("uzgadnianie WebSocket: {0}")]
    Handshake(String),
    /// Ramka spoza protokołu.
    #[error("protokół WebSocket: {0}")]
    Protocol(String),
    /// Wiadomość przekracza limit.
    #[error("wiadomość ponad limit {0} B")]
    TooLarge(usize),
    /// Serwer zamknął połączenie.
    #[error("serwer zamknął połączenie")]
    Closed,
}

fn io_err(e: std::io::Error) -> WsError {
    WsError::Io(e.to_string())
}

/// Połączenie po uzgodnieniu.
pub(crate) struct WsStream<S> {
    io: S,
    max_message: usize,
}

impl<S: AsyncRead + AsyncWrite + Unpin> WsStream<S> {
    /// Uzgadnia połączenie (`host` — `127.0.0.1:port`, `path` — zwykle `/`).
    pub(crate) async fn handshake(
        mut io: S,
        host: &str,
        path: &str,
        max_message: usize,
    ) -> Result<Self, WsError> {
        let mut nonce = [0u8; 16];
        getrandom::fill(&mut nonce).map_err(|e| WsError::Io(format!("losowanie: {e}")))?;
        let key = base64::engine::general_purpose::STANDARD.encode(nonce);
        let request = format!(
            "GET {path} HTTP/1.1\r\nHost: {host}\r\nUpgrade: websocket\r\nConnection: Upgrade\r\n\
             Sec-WebSocket-Key: {key}\r\nSec-WebSocket-Version: 13\r\n\r\n"
        );
        io.write_all(request.as_bytes()).await.map_err(io_err)?;
        let mut head = Vec::new();
        let mut byte = [0u8; 1];
        while !head.ends_with(b"\r\n\r\n") {
            if head.len() >= MAX_HEADERS {
                return Err(WsError::Handshake("za długie nagłówki odpowiedzi".into()));
            }
            if io.read(&mut byte).await.map_err(io_err)? == 0 {
                return Err(WsError::Handshake("serwer zamknął połączenie".into()));
            }
            head.push(byte[0]);
        }
        let head = String::from_utf8_lossy(&head);
        let mut lines = head.split("\r\n");
        let status = lines.next().unwrap_or_default();
        if status.split_whitespace().nth(1) != Some("101") {
            return Err(WsError::Handshake(format!("odpowiedź „{}”", status.trim())));
        }
        let expected = base64::engine::general_purpose::STANDARD
            .encode(sha1(format!("{key}{GUID}").as_bytes()));
        let accept = lines.find_map(|l| {
            let (name, value) = l.split_once(':')?;
            name.trim()
                .eq_ignore_ascii_case("sec-websocket-accept")
                .then(|| value.trim().to_owned())
        });
        if accept.as_deref() != Some(expected.as_str()) {
            return Err(WsError::Handshake("zły Sec-WebSocket-Accept".into()));
        }
        Ok(Self { io, max_message })
    }

    /// Wysyła wiadomość tekstową (ramka maskowana, jak każe RFC dla klienta).
    pub(crate) async fn send_text(&mut self, text: &str) -> Result<(), WsError> {
        self.send_frame(0x1, text.as_bytes()).await
    }

    async fn send_frame(&mut self, opcode: u8, payload: &[u8]) -> Result<(), WsError> {
        let mut frame = Vec::with_capacity(payload.len() + 14);
        frame.push(0x80 | opcode);
        let len = payload.len();
        if len < 126 {
            frame.push(0x80 | len as u8);
        } else if let Ok(short) = u16::try_from(len) {
            frame.push(0x80 | 126);
            frame.extend_from_slice(&short.to_be_bytes());
        } else {
            frame.push(0x80 | 127);
            frame.extend_from_slice(&(len as u64).to_be_bytes());
        }
        let mut mask = [0u8; 4];
        getrandom::fill(&mut mask).map_err(|e| WsError::Io(format!("losowanie: {e}")))?;
        frame.extend_from_slice(&mask);
        frame.extend(payload.iter().enumerate().map(|(i, b)| b ^ mask[i % 4]));
        self.io.write_all(&frame).await.map_err(io_err)?;
        self.io.flush().await.map_err(io_err)
    }

    /// Czeka na następną wiadomość tekstową; odpowiada na ping, łączy fragmenty.
    pub(crate) async fn recv_text(&mut self) -> Result<String, WsError> {
        let mut message: Vec<u8> = Vec::new();
        let mut in_message = false;
        loop {
            let (fin, opcode, payload) = self.read_frame().await?;
            match opcode {
                0x9 => self.send_frame(0xA, &payload).await?,
                0xA => {}
                0x8 => {
                    // Odpowiedź na zamknięcie; błąd zapisu bez znaczenia — i tak kończymy.
                    let _ = self.send_frame(0x8, &payload).await;
                    return Err(WsError::Closed);
                }
                // Serwer silnika wysyła tylko tekst; ramka binarna to błąd protokołu.
                0x0 | 0x1 => {
                    if (opcode == 0x0) != in_message {
                        return Err(WsError::Protocol("nieoczekiwany fragment".into()));
                    }
                    if message.len() + payload.len() > self.max_message {
                        return Err(WsError::TooLarge(self.max_message));
                    }
                    message.extend_from_slice(&payload);
                    in_message = !fin;
                    if fin {
                        return String::from_utf8(message)
                            .map_err(|_| WsError::Protocol("wiadomość nie jest UTF-8".into()));
                    }
                }
                other => return Err(WsError::Protocol(format!("kod ramki {other:#x}"))),
            }
        }
    }

    async fn read_frame(&mut self) -> Result<(bool, u8, Vec<u8>), WsError> {
        let mut head = [0u8; 2];
        self.io.read_exact(&mut head).await.map_err(io_err)?;
        let fin = head[0] & 0x80 != 0;
        if head[0] & 0x70 != 0 {
            return Err(WsError::Protocol("bity RSV bez rozszerzeń".into()));
        }
        let opcode = head[0] & 0x0F;
        if head[1] & 0x80 != 0 {
            return Err(WsError::Protocol("serwer zamaskował ramkę".into()));
        }
        let len = match head[1] & 0x7F {
            126 => {
                let mut b = [0u8; 2];
                self.io.read_exact(&mut b).await.map_err(io_err)?;
                u64::from(u16::from_be_bytes(b))
            }
            127 => {
                let mut b = [0u8; 8];
                self.io.read_exact(&mut b).await.map_err(io_err)?;
                u64::from_be_bytes(b)
            }
            n => u64::from(n),
        };
        // RFC 6455 §5.5: ramki sterujące niepodzielone i do 125 bajtów — sprawdzane przed odczytem
        // treści, więc ping z 1 MiB nie wraca w całości jako pong (przegląd 2026-10-27).
        if opcode >= 0x8 && (!fin || len > 125) {
            return Err(WsError::Protocol(format!(
                "ramka sterująca {opcode:#x} niezgodna z RFC 6455"
            )));
        }
        let len = usize::try_from(len)
            .ok()
            .filter(|l| *l <= self.max_message)
            .ok_or(WsError::TooLarge(self.max_message))?;
        let mut payload = vec![0u8; len];
        self.io.read_exact(&mut payload).await.map_err(io_err)?;
        Ok((fin, opcode, payload))
    }

    /// Zamyka połączenie (kod 1000).
    pub(crate) async fn close(&mut self) {
        let _ = self.send_frame(0x8, &1000u16.to_be_bytes()).await;
        let _ = self.io.shutdown().await;
    }
}

#[cfg(test)]
mod tests {
    use base64::Engine as _;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::{TcpListener, TcpStream};

    use super::{GUID, WsError, WsStream};
    use crate::sha1::sha1;

    /// Serwer testowy: uzgadnianie (opcjonalnie z błędnym Accept), potem odbiór jednej ramki
    /// klienta i odpowiedź: ping, wiadomość w dwóch fragmentach, zamknięcie.
    async fn server(listener: TcpListener, good_accept: bool) -> Option<String> {
        let (mut s, _) = listener.accept().await.unwrap();
        let mut head = Vec::new();
        let mut b = [0u8; 1];
        while !head.ends_with(b"\r\n\r\n") {
            s.read_exact(&mut b).await.unwrap();
            head.push(b[0]);
        }
        let head = String::from_utf8(head).unwrap();
        assert!(
            !head.to_ascii_lowercase().contains("origin:"),
            "klient nie wysyła Origin"
        );
        let key = head
            .lines()
            .find_map(|l| l.strip_prefix("Sec-WebSocket-Key: "))
            .unwrap()
            .trim()
            .to_owned();
        let accept = if good_accept {
            base64::engine::general_purpose::STANDARD
                .encode(sha1(format!("{key}{GUID}").as_bytes()))
        } else {
            "zly".into()
        };
        let reply = format!(
            "HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: {accept}\r\n\r\n"
        );
        s.write_all(reply.as_bytes()).await.unwrap();
        if !good_accept {
            return None;
        }
        let mut h = [0u8; 2];
        s.read_exact(&mut h).await.unwrap();
        assert_eq!(h[0], 0x81);
        assert!(h[1] & 0x80 != 0, "ramka klienta maskowana");
        let len = usize::from(h[1] & 0x7F);
        let mut mask = [0u8; 4];
        s.read_exact(&mut mask).await.unwrap();
        let mut payload = vec![0u8; len];
        s.read_exact(&mut payload).await.unwrap();
        let text: Vec<u8> = payload
            .iter()
            .enumerate()
            .map(|(i, x)| x ^ mask[i % 4])
            .collect();
        // Ping, potem odpowiedź w dwóch fragmentach, potem zamknięcie.
        s.write_all(&[0x89, 0x01, b'p']).await.unwrap();
        s.write_all(&[0x01, 0x03, b'{', b'"', b'a']).await.unwrap();
        s.write_all(&[0x80, 0x04, b'"', b':', b'1', b'}'])
            .await
            .unwrap();
        let mut pong = [0u8; 7];
        s.read_exact(&mut pong).await.unwrap();
        assert_eq!(pong[0], 0x8A, "odpowiedź pong na ping");
        s.write_all(&[0x88, 0x00]).await.unwrap();
        Some(String::from_utf8(text).unwrap())
    }

    #[tokio::test]
    async fn handshake_text_ping_fragments_and_close() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap().to_string();
        let srv = tokio::spawn(server(listener, true));
        let tcp = TcpStream::connect(&addr).await.unwrap();
        let mut ws = WsStream::handshake(tcp, &addr, "/", 1024).await.unwrap();
        ws.send_text("{\"x\":1}").await.unwrap();
        assert_eq!(ws.recv_text().await.unwrap(), "{\"a\":1}");
        assert_eq!(ws.recv_text().await, Err(WsError::Closed));
        assert_eq!(srv.await.unwrap().as_deref(), Some("{\"x\":1}"));
    }

    #[tokio::test]
    async fn wrong_accept_is_refused() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap().to_string();
        let srv = tokio::spawn(server(listener, false));
        let tcp = TcpStream::connect(&addr).await.unwrap();
        let err = WsStream::handshake(tcp, &addr, "/", 1024)
            .await
            .err()
            .unwrap();
        assert!(matches!(err, WsError::Handshake(_)));
        srv.await.unwrap();
    }

    /// Serwer testowy bez sprawdzania nagłówków: poprawne uzgodnienie, potem podane bajty ramek.
    async fn raw_server(listener: TcpListener, frames: &'static [u8]) {
        let (mut s, _) = listener.accept().await.unwrap();
        let mut head = Vec::new();
        let mut b = [0u8; 1];
        while !head.ends_with(b"\r\n\r\n") {
            s.read_exact(&mut b).await.unwrap();
            head.push(b[0]);
        }
        let head = String::from_utf8(head).unwrap();
        let key = head
            .lines()
            .find_map(|l| l.strip_prefix("Sec-WebSocket-Key: "))
            .unwrap()
            .trim()
            .to_owned();
        let accept = base64::engine::general_purpose::STANDARD
            .encode(sha1(format!("{key}{GUID}").as_bytes()));
        let reply = format!("HTTP/1.1 101 OK\r\nSec-WebSocket-Accept: {accept}\r\n\r\n");
        s.write_all(reply.as_bytes()).await.unwrap();
        s.write_all(frames).await.unwrap();
        // Gniazdo otwarte do rozłączenia klienta: wcześniejsze zamknięcie na Windows (RST) gubi
        // jeszcze nieodczytane ramki.
        let _ = s.read(&mut b).await;
    }

    async fn first_reply(frames: &'static [u8], limit: usize) -> Result<String, WsError> {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap().to_string();
        let srv = tokio::spawn(raw_server(listener, frames));
        let tcp = TcpStream::connect(&addr).await.unwrap();
        let mut ws = WsStream::handshake(tcp, &addr, "/", limit).await.unwrap();
        let reply = ws.recv_text().await;
        drop(ws);
        srv.await.unwrap();
        reply
    }

    #[tokio::test]
    async fn oversized_and_masked_server_frames_are_errors() {
        // Długość 200 przy limicie 64.
        assert_eq!(
            first_reply(&[0x81, 126, 0x00, 200], 64).await,
            Err(WsError::TooLarge(64))
        );
        assert!(matches!(
            first_reply(&[0x81, 0x81, 0, 0, 0, 0, b'x'], 64).await,
            Err(WsError::Protocol(_))
        ));
    }

    /// Przegląd 2026-10-27: podzielony ping, ping ponad 125 bajtów i ramka binarna to błędy.
    #[tokio::test]
    async fn control_frames_follow_rfc_6455() {
        let fragmented_ping: &[u8] = &[0x09, 0x01, b'p'];
        assert!(matches!(
            first_reply(fragmented_ping, 1024).await,
            Err(WsError::Protocol(_))
        ));
        let long_ping: &[u8] = &[0x89, 126, 0x00, 126];
        assert!(matches!(
            first_reply(long_ping, 1024).await,
            Err(WsError::Protocol(_))
        ));
        let binary: &[u8] = &[0x82, 0x01, b'x'];
        assert!(matches!(
            first_reply(binary, 1024).await,
            Err(WsError::Protocol(_))
        ));
    }
}
