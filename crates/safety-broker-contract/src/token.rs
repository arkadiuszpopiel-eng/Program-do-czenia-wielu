//! Token zdolności i jego kanoniczny format przewodowy.
//!
//! Format (bajty, big-endian): `ALFT` | wersja | id | rodzic? | boot (16 B) | epoka klucza |
//! wydany | wygasa | sesja | agentka? | rola? | zdolność (kanoniczny JSON) | MAC (32 B).
//! MAC (HMAC-SHA256 kluczem Brokera) obejmuje wszystko przed nim. Parser jest ścisły: po
//! odczycie koduje ciało ponownie i wymaga identycznych bajtów, więc każda zmiana dowolnego
//! bajtu daje błąd parsowania albo inne ciało → niezgodny MAC.

use core_bus_contract::{AgentId, SessionId};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::capability::Capability;
use crate::hex;

const MAGIC: &[u8; 4] = b"ALFT";
const VERSION: u8 = 1;
/// Maksymalny rozmiar tokenu w bajtach.
pub const MAX_TOKEN_BYTES: usize = 8 * 1024;
/// Długość MAC (HMAC-SHA256).
pub const MAC_LEN: usize = 32;

/// Identyfikator tokenu (unikatowy w obrębie uruchomienia Brokera).
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
pub struct TokenId(pub u64);

/// Identyfikator uruchomienia Brokera (losowy) — token nie przechodzi między uruchomieniami.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct BootId(pub [u8; 16]);

/// Podmiot tokenu: sesja, opcjonalnie agentka i rola (uprawnienia idą za rolą, ADR 15).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
pub struct Holder {
    /// Sesja.
    pub session: SessionId,
    /// Agentka (persona).
    pub agent: Option<AgentId>,
    /// Rola w obsadzie (np. `wykonawczyni`).
    pub role: Option<String>,
}

impl Holder {
    /// Podmiot: sesja + agentka.
    pub fn agent(session: &str, agent: &str) -> Self {
        Self {
            session: SessionId::new(session),
            agent: Some(AgentId::new(agent)),
            role: None,
        }
    }

    /// Ustawia rolę (builder).
    #[must_use]
    pub fn with_role(mut self, role: &str) -> Self {
        self.role = Some(role.to_owned());
        self
    }
}

/// Token zdolności. Serializowany jako hex formatu przewodowego (jedna postać).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(try_from = "String", into = "String")]
#[schemars(with = "String")]
pub struct CapToken {
    /// Identyfikator.
    pub id: TokenId,
    /// Token-rodzic (atenuacja).
    pub parent: Option<TokenId>,
    /// Zdolność z zakresem.
    pub cap: Capability,
    /// Podmiot.
    pub holder: Holder,
    /// Uruchomienie Brokera.
    pub boot: BootId,
    /// Epoka klucza MAC (rotacja).
    pub key_epoch: u32,
    /// Wydany (ms).
    pub issued_at_ms: u64,
    /// Wygasa (ms, wyłącznie) — TTL zawsze skończony.
    pub expires_at_ms: u64,
    /// HMAC-SHA256 ciała.
    pub mac: [u8; MAC_LEN],
}

/// Błąd formatu przewodowego.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("uszkodzony token: {0}")]
pub struct WireError(pub String);

fn put_str(out: &mut Vec<u8>, s: &str) {
    let len = u16::try_from(s.len()).unwrap_or(u16::MAX);
    out.extend_from_slice(&len.to_be_bytes());
    out.extend_from_slice(&s.as_bytes()[..usize::from(len)]);
}

fn put_opt(out: &mut Vec<u8>, s: Option<&str>) {
    match s {
        Some(s) => {
            out.push(1);
            put_str(out, s);
        }
        None => out.push(0),
    }
}

impl CapToken {
    /// Kanoniczne ciało (wszystko poza MAC) — wejście HMAC.
    pub fn signing_bytes(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(160);
        out.extend_from_slice(MAGIC);
        out.push(VERSION);
        out.extend_from_slice(&self.id.0.to_be_bytes());
        match self.parent {
            Some(p) => {
                out.push(1);
                out.extend_from_slice(&p.0.to_be_bytes());
            }
            None => out.push(0),
        }
        out.extend_from_slice(&self.boot.0);
        out.extend_from_slice(&self.key_epoch.to_be_bytes());
        out.extend_from_slice(&self.issued_at_ms.to_be_bytes());
        out.extend_from_slice(&self.expires_at_ms.to_be_bytes());
        put_str(&mut out, self.holder.session.as_str());
        put_opt(&mut out, self.holder.agent.as_ref().map(AgentId::as_str));
        put_opt(&mut out, self.holder.role.as_deref());
        let cap = serde_json::to_vec(&self.cap).unwrap_or_default();
        let len = u32::try_from(cap.len()).unwrap_or(u32::MAX);
        out.extend_from_slice(&len.to_be_bytes());
        out.extend_from_slice(&cap);
        out
    }

    /// Format przewodowy: ciało + MAC.
    pub fn to_wire(&self) -> Vec<u8> {
        let mut out = self.signing_bytes();
        out.extend_from_slice(&self.mac);
        out
    }

    /// Ścisły parser formatu przewodowego (nie sprawdza MAC — to robi Broker).
    pub fn from_wire(bytes: &[u8]) -> Result<Self, WireError> {
        if bytes.len() > MAX_TOKEN_BYTES || bytes.len() < MAC_LEN {
            return Err(WireError("nieprawidłowa długość".into()));
        }
        let (body, mac) = bytes.split_at(bytes.len() - MAC_LEN);
        let mut r = Reader { buf: body, pos: 0 };
        if r.take(4)? != MAGIC || r.u8()? != VERSION {
            return Err(WireError("nieznany format lub wersja".into()));
        }
        let id = TokenId(r.u64()?);
        let parent = match r.u8()? {
            0 => None,
            1 => Some(TokenId(r.u64()?)),
            _ => return Err(WireError("flaga rodzica".into())),
        };
        let boot = BootId(r.array::<16>()?);
        let key_epoch = u32::from_be_bytes(r.array::<4>()?);
        let issued_at_ms = r.u64()?;
        let expires_at_ms = r.u64()?;
        let session = SessionId::new(r.string()?);
        let agent = r.opt_string()?.map(AgentId::new);
        let role = r.opt_string()?;
        let cap_len = usize::try_from(u32::from_be_bytes(r.array::<4>()?))
            .map_err(|_| WireError("długość zdolności".into()))?;
        let cap: Capability = serde_json::from_slice(r.take(cap_len)?)
            .map_err(|e| WireError(format!("zdolność: {e}")))?;
        if r.pos != body.len() {
            return Err(WireError("nadmiarowe bajty".into()));
        }
        let token = Self {
            id,
            parent,
            cap,
            holder: Holder {
                session,
                agent,
                role,
            },
            boot,
            key_epoch,
            issued_at_ms,
            expires_at_ms,
            mac: mac
                .try_into()
                .map_err(|_| WireError("długość MAC".into()))?,
        };
        if token.signing_bytes() != body {
            return Err(WireError("postać niekanoniczna".into()));
        }
        Ok(token)
    }

    /// Pozostały czas życia (0 = wygasł).
    pub fn remaining_ms(&self, now_ms: u64) -> u64 {
        self.expires_at_ms.saturating_sub(now_ms)
    }
}

impl TryFrom<String> for CapToken {
    type Error = WireError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        let bytes = hex::decode(&value).ok_or_else(|| WireError("niepoprawny hex".into()))?;
        Self::from_wire(&bytes)
    }
}

impl From<CapToken> for String {
    fn from(value: CapToken) -> Self {
        hex::encode(&value.to_wire())
    }
}

struct Reader<'a> {
    buf: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8], WireError> {
        let end = self
            .pos
            .checked_add(n)
            .filter(|end| *end <= self.buf.len())
            .ok_or_else(|| WireError("za krótki".into()))?;
        let out = &self.buf[self.pos..end];
        self.pos = end;
        Ok(out)
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N], WireError> {
        self.take(N)?
            .try_into()
            .map_err(|_| WireError("za krótki".into()))
    }

    fn u8(&mut self) -> Result<u8, WireError> {
        Ok(self.array::<1>()?[0])
    }

    fn u64(&mut self) -> Result<u64, WireError> {
        Ok(u64::from_be_bytes(self.array::<8>()?))
    }

    fn string(&mut self) -> Result<String, WireError> {
        let len = usize::from(u16::from_be_bytes(self.array::<2>()?));
        let raw = self.take(len)?;
        String::from_utf8(raw.to_vec()).map_err(|_| WireError("niepoprawny UTF-8".into()))
    }

    fn opt_string(&mut self) -> Result<Option<String>, WireError> {
        match self.u8()? {
            0 => Ok(None),
            1 => self.string().map(Some),
            _ => Err(WireError("flaga pola opcjonalnego".into())),
        }
    }
}
