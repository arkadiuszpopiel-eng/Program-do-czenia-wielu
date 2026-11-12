//! Czysty format polecenia klient–host (ADR 0019), bez sieci, parowania i ACL.
//!
//! Dekoder nie uwierzytelnia klienta ani nie uprawnia do wywołania komendy. Przyszły transport
//! musi najpierw sprawdzić kanał E2E, następnie użyć osobnego dekodera dla połączenia, a przed
//! wykonaniem sprawdzić rejestr komend, ACL, `local_only`, argumenty DTO i Broker.

use std::fmt;
use std::num::{NonZeroU16, NonZeroUsize};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

/// Największy całkowity identyfikator dokładnie reprezentowany w JavaScript.
pub const MAX_REQUEST_ID: u64 = (1_u64 << 53) - 1;

/// Format polecenia, nie dowód tożsamości ani zgody. Nieznane pola są błędem.
#[derive(Clone, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CommandFrame {
    /// Wersja wynegocjowana dla połączenia.
    pub v: u16,
    /// Ściśle rosnący identyfikator w zakresie 1..=2^53−1.
    #[schemars(range(min = 1, max = 9_007_199_254_740_991_u64))]
    pub id: u64,
    /// Nazwa z rejestru komend; parser nie rozstrzyga jej uprawnień.
    pub cmd: String,
    /// Argumenty obiektowe; ich typ sprawdza odbiorca konkretnej komendy.
    pub args: Map<String, Value>,
}

impl fmt::Debug for CommandFrame {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CommandFrame")
            .field("v", &self.v)
            .field("id", &self.id)
            .finish_non_exhaustive()
    }
}

/// Stały błąd formatu, bez treści polecenia, argumentów ani błędu parsera JSON.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum DecodeError {
    /// Połączenie już zamknięte.
    #[error("Połączenie jest zamknięte")]
    Closed,
    /// Limit narzucony przez host przekroczony przed parsowaniem JSON.
    #[error("Ramka przekracza budżet bajtów")]
    TooLarge,
    /// JSON nie odpowiada ścisłemu formatowi ramki.
    #[error("Nieprawidłowy format ramki")]
    Malformed,
    /// Wersja różni się od wynegocjowanej.
    #[error("Niezgodna wersja protokołu")]
    Version,
    /// Zero albo liczba poza dokładnym zakresem JavaScript.
    #[error("Nieprawidłowy identyfikator polecenia")]
    RequestId,
    /// Identyfikator został już zużyty albo jest starszy.
    #[error("Powtórzony identyfikator polecenia")]
    Replay,
}

/// Obsługiwane wersje: bieżąca i poprzednia, z jawnym minimum hosta.
#[derive(Debug, Clone, Copy)]
pub struct ProtocolVersions {
    current: u16,
    minimum: u16,
}

impl ProtocolVersions {
    /// Brak konfiguracji, jeśli minimum przekracza bieżącą wersję.
    pub fn new(current: NonZeroU16, minimum: NonZeroU16) -> Option<Self> {
        (minimum <= current).then_some(Self {
            current: current.get(),
            minimum: minimum.get(),
        })
    }

    /// Najwyższa wspólna wersja, bez cichego zejścia poniżej minimum.
    pub fn negotiate(&self, offered: &[u16]) -> Option<NonZeroU16> {
        offered
            .iter()
            .copied()
            .filter(|v| *v >= self.minimum && *v <= self.current && self.current - *v <= 1)
            .max()
            .and_then(NonZeroU16::new)
    }
}

/// Dekoder jednego połączenia. Limit jest obowiązkowy — biblioteka nie ustala budżetu hosta.
pub struct CommandDecoder {
    version: NonZeroU16,
    max_bytes: NonZeroUsize,
    last_id: u64,
    closed: bool,
}

impl CommandDecoder {
    /// Wywołuje host po negocjacji; nie zastępuje to uwierzytelniania kanału.
    pub fn new(version: NonZeroU16, max_bytes: NonZeroUsize) -> Self {
        Self {
            version,
            max_bytes,
            last_id: 0,
            closed: false,
        }
    }

    /// Parsuje ramkę; wynik nadal wymaga kontroli uprawnień i argumentów przed wykonaniem.
    pub fn decode(&mut self, bytes: &[u8]) -> Result<CommandFrame, DecodeError> {
        if self.closed {
            return Err(DecodeError::Closed);
        }
        if bytes.len() > self.max_bytes.get() {
            return Err(DecodeError::TooLarge);
        }
        let frame: CommandFrame =
            serde_json::from_slice(bytes).map_err(|_| DecodeError::Malformed)?;
        if frame.v != self.version.get() {
            return Err(DecodeError::Version);
        }
        if frame.id == 0 || frame.id > MAX_REQUEST_ID {
            return Err(DecodeError::RequestId);
        }
        if frame.id <= self.last_id {
            return Err(DecodeError::Replay);
        }
        // Zużycie przed przyszłym dispatcherem: także polecenie odrzucone przez ACL/Broker
        // nie może wrócić pod starym ID po zmianie uprawnień w tym samym kanale.
        self.last_id = frame.id;
        Ok(frame)
    }

    /// Zamyka dekoder bez możliwości ponownego otwarcia; nowy kanał wymaga nowej instancji.
    pub fn close(&mut self) {
        self.closed = true;
    }
}
