//! Synchroniczny klient protokołu IPC Brokera nad dowolnym `Read + Write` (named pipe z ACL
//! z `platform-contract::SecurePipePort`). Używany przez Broker-UI i watchdoga — procesy bez
//! środowiska async. Ramki i semantyka jak w [`crate::ipc`]; tu wyłącznie we/wy blokujące.
//!
//! Bilet startowy Broker-UI ([`UiLaunchTicket`]) Broker przekazuje przez stdin procesu (nigdy
//! przez wiersz poleceń ani plik): poświadczenie roli `BrokerUi`, nazwę potoku i oczekiwaną
//! tożsamość serwera (ochrona przed podstawionym serwerem, gdy usługa nie działa).

use std::io::{ErrorKind, Read, Write};

use schemars::JsonSchema;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use crate::ipc::{
    ClientCredential, Envelope, FrameError, Hello, HelloReply, MAX_FRAME_BYTES, PROTOCOL_VERSION,
    Request, Response, decode_body, encode_frame, frame_len,
};

/// Bilet startowy Broker-UI (jedna linia JSON na stdin).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct UiLaunchTicket {
    /// Poświadczenie roli `BrokerUi`.
    pub credential: ClientCredential,
    /// Nazwa potoku Brokera (bez `\\.\pipe\`).
    pub pipe: String,
    /// SID konta usługi Brokera — klient sprawdza, że serwer potoku działa na tym koncie.
    pub broker_user: Option<String>,
}

/// Błąd klienta blokującego.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum BlockingError {
    /// Błąd ramki.
    #[error(transparent)]
    Frame(#[from] FrameError),
    /// Błąd strumienia.
    #[error("strumień IPC: {0}")]
    Io(String),
    /// Serwer odrzucił powitanie.
    #[error("połączenie odrzucone: {0}")]
    Rejected(String),
    /// Druga strona zamknęła połączenie.
    #[error("połączenie zamknięte")]
    Closed,
}

/// Czyta jedną ramkę; `Ok(None)` przy czystym końcu strumienia przed nagłówkiem.
pub fn read_frame<R: Read, T: DeserializeOwned>(r: &mut R) -> Result<Option<T>, BlockingError> {
    let mut header = [0u8; 4];
    match r.read_exact(&mut header) {
        Ok(()) => {}
        Err(e) if e.kind() == ErrorKind::UnexpectedEof => return Ok(None),
        Err(e) => return Err(BlockingError::Io(e.to_string())),
    }
    let len = frame_len(header)?;
    let mut body = vec![0u8; len.min(MAX_FRAME_BYTES)];
    r.read_exact(&mut body)
        .map_err(|e| BlockingError::Io(e.to_string()))?;
    Ok(Some(decode_body(&body)?))
}

/// Zapisuje jedną ramkę (jednym `write_all` — potok w trybie bajtowym).
pub fn write_frame<W: Write, T: Serialize>(w: &mut W, msg: &T) -> Result<(), BlockingError> {
    let frame = encode_frame(msg)?;
    w.write_all(&frame)
        .and_then(|()| w.flush())
        .map_err(|e| BlockingError::Io(e.to_string()))
}

/// Klient blokujący: powitanie, potem żądanie → odpowiedź o tym samym numerze.
#[derive(Debug)]
pub struct BlockingClient<S> {
    stream: S,
    next_id: u64,
}

impl<S: Read + Write> BlockingClient<S> {
    /// Wysyła powitanie i czeka na przyjęcie.
    pub fn connect(mut stream: S, hello: &Hello) -> Result<Self, BlockingError> {
        write_frame(&mut stream, hello)?;
        match read_frame::<_, HelloReply>(&mut stream)? {
            Some(HelloReply::Welcome { .. }) => Ok(Self { stream, next_id: 0 }),
            Some(HelloReply::Rejected { reason }) => Err(BlockingError::Rejected(reason)),
            None => Err(BlockingError::Closed),
        }
    }

    /// Wywołuje żądanie.
    pub fn call(&mut self, body: Request) -> Result<Response, BlockingError> {
        self.next_id += 1;
        let id = self.next_id;
        let env = Envelope {
            v: PROTOCOL_VERSION,
            id,
            body,
        };
        write_frame(&mut self.stream, &env)?;
        match read_frame::<_, Envelope<Response>>(&mut self.stream)? {
            Some(env) if env.id == id => Ok(env.body),
            Some(_) => Err(BlockingError::Io("odpowiedź na inne żądanie".into())),
            None => Err(BlockingError::Closed),
        }
    }

    /// Strumień (np. do odczytu PID serwera).
    pub fn stream(&self) -> &S {
        &self.stream
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ipc::{ClientRole, encode_frame};
    use std::io::Cursor;

    struct Duplex {
        input: Cursor<Vec<u8>>,
        output: Vec<u8>,
    }

    impl Read for Duplex {
        fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
            self.input.read(buf)
        }
    }

    impl Write for Duplex {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.output.write(buf)
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    fn hello() -> Hello {
        Hello {
            protocol: PROTOCOL_VERSION,
            credential: ClientCredential {
                client_id: "ui".into(),
                role: ClientRole::BrokerUi,
                expires_at_ms: 1,
                mac: String::new(),
            },
            pid: 1,
            sid: None,
            image: None,
        }
    }

    fn scripted(replies: &[Vec<u8>]) -> Duplex {
        Duplex {
            input: Cursor::new(replies.concat()),
            output: Vec::new(),
        }
    }

    #[test]
    fn handshake_and_call() {
        let welcome = encode_frame(&HelloReply::Welcome { protocol: 1 }).unwrap();
        let reply = encode_frame(&Envelope {
            v: 1,
            id: 1,
            body: Response::Ok,
        })
        .unwrap();
        let wrong = encode_frame(&Envelope {
            v: 1,
            id: 9,
            body: Response::Ok,
        })
        .unwrap();
        let mut c = BlockingClient::connect(scripted(&[welcome, reply, wrong]), &hello()).unwrap();
        assert_eq!(c.call(Request::Metrics).unwrap(), Response::Ok);
        assert!(matches!(
            c.call(Request::Metrics),
            Err(BlockingError::Io(_))
        ));
        assert_eq!(c.call(Request::Metrics), Err(BlockingError::Closed));
        let sent: Option<Hello> = read_frame(&mut Cursor::new(c.stream().output.clone())).unwrap();
        assert_eq!(sent, Some(hello()));
    }

    #[test]
    fn rejection_garbage_and_eof() {
        let rejected = encode_frame(&HelloReply::Rejected {
            reason: "nie".into(),
        })
        .unwrap();
        assert_eq!(
            BlockingClient::connect(scripted(&[rejected]), &hello()).err(),
            Some(BlockingError::Rejected("nie".into()))
        );
        assert_eq!(
            BlockingClient::connect(scripted(&[]), &hello()).err(),
            Some(BlockingError::Closed)
        );
        let mut huge = (64 * 1024 * 1024u32).to_le_bytes().to_vec();
        huge.extend_from_slice(b"x");
        assert!(matches!(
            read_frame::<_, Response>(&mut Cursor::new(huge)),
            Err(BlockingError::Frame(FrameError::TooLarge(_)))
        ));
        let mut cut = 10u32.to_le_bytes().to_vec();
        cut.extend_from_slice(b"{}");
        assert!(matches!(
            read_frame::<_, Response>(&mut Cursor::new(cut)),
            Err(BlockingError::Io(_))
        ));
        let ticket = UiLaunchTicket {
            credential: hello().credential,
            pipe: "alfa-broker".into(),
            broker_user: Some("S-1-5-18".into()),
        };
        let json = serde_json::to_string(&ticket).unwrap();
        assert_eq!(
            serde_json::from_str::<UiLaunchTicket>(&json).unwrap(),
            ticket
        );
    }
}
