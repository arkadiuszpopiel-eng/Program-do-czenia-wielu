//! Dowód fizycznego wejścia z Broker-UI (PLAN §8.2, ADR 3).
//!
//! [`PhysicalInputProof`] nie ma publicznych pól, nie ma `Default`, `Clone` ani `Deserialize`.
//! Jedyny konstruktor jest ukryty w module [`broker_ui_only`] i przeznaczony wyłącznie dla
//! Broker-UI (w części 2: natywny proces na wyższym poziomie integralności; tutaj — umowa
//! egzekwowana przeglądem, jak `KernelAuthority`). Siłę dowodu daje Broker, nie typ:
//! dowód musi nieść **jednorazowy nonce wyzwania**, które Broker wysyła wyłącznie kanałem
//! Broker-UI, wejście nie może być wstrzyknięte (`LLMHF_INJECTED`) i musi być świeże.
//!
//! Poza crate'em nie da się zbudować dowodu literałem struktury:
//! ```compile_fail,E0451
//! use safety_broker_contract::{PhysicalInputProof, InputSource, Nonce, ApprovalId};
//! let forged = PhysicalInputProof {
//!     approval: ApprovalId(1),
//!     nonce: Nonce([0; 16]),
//!     source: InputSource::MouseClick,
//!     injected: false,
//!     at_ms: 0,
//! };
//! ```
//! ani zmienić pól otrzymanego dowodu:
//! ```compile_fail,E0616
//! fn tamper(p: &mut safety_broker_contract::PhysicalInputProof) {
//!     p.injected = false;
//! }
//! ```
//! ani zdeserializować go z JSON-a (brak `Deserialize`):
//! ```compile_fail,E0277
//! let p: safety_broker_contract::PhysicalInputProof = serde_json::from_str("{}").unwrap();
//! ```
//! ani sklonować (brak `Clone`) — każdy dowód jest zużywany raz:
//! ```compile_fail,E0308
//! fn dup(p: &safety_broker_contract::PhysicalInputProof) -> safety_broker_contract::PhysicalInputProof {
//!     p.clone()
//! }
//! ```

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::approval::ApprovalId;
use crate::hex;

/// Jednorazowy nonce wyzwania (128 bitów, losowany przez Brokera).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(try_from = "String", into = "String")]
#[schemars(with = "String")]
pub struct Nonce(pub [u8; 16]);

impl TryFrom<String> for Nonce {
    type Error = String;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        hex::decode_array::<16>(&value)
            .map(Nonce)
            .ok_or_else(|| "nonce: oczekiwano 32 znaków hex".to_owned())
    }
}

impl From<Nonce> for String {
    fn from(value: Nonce) -> Self {
        hex::encode(&value.0)
    }
}

/// Źródło fizycznego wejścia. Brak wariantu „głos” — potwierdzenie z Broker-UI jest zawsze
/// nie-głosowe (§6.10).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum InputSource {
    /// Kliknięcie myszą.
    MouseClick,
    /// Klawisz.
    Keyboard,
    /// Windows Hello (PIN/biometria) — wymagane opcjonalnie dla L4, admina, polityk.
    WindowsHello,
}

/// Dowód fizycznego wejścia właściciela w Broker-UI.
#[derive(Debug, PartialEq, Eq)]
pub struct PhysicalInputProof {
    approval: ApprovalId,
    nonce: Nonce,
    source: InputSource,
    injected: bool,
    at_ms: u64,
}

impl PhysicalInputProof {
    /// Prośba, której dotyczy.
    pub fn approval(&self) -> ApprovalId {
        self.approval
    }

    /// Nonce wyzwania.
    pub fn nonce(&self) -> Nonce {
        self.nonce
    }

    /// Źródło wejścia.
    pub fn source(&self) -> InputSource {
        self.source
    }

    /// Czy system oznaczył wejście jako wstrzyknięte (SendInput) — Broker odrzuca.
    pub fn injected(&self) -> bool {
        self.injected
    }

    /// Chwila wejścia (ms).
    pub fn at_ms(&self) -> u64 {
        self.at_ms
    }
}

/// WYŁĄCZNIE dla Broker-UI (i serwera IPC Brokera dla połączeń uwierzytelnionych jako
/// Broker-UI). Użycie gdziekolwiek indziej jest błędem przeglądu kodu.
#[doc(hidden)]
pub mod broker_ui_only {
    use super::{ApprovalId, InputSource, Nonce, PhysicalInputProof};

    /// Buduje dowód z danych zdarzenia wejścia (flaga `injected` z `LLMHF_INJECTED`/`LLKHF_INJECTED`).
    pub fn physical_input_proof(
        approval: ApprovalId,
        nonce: Nonce,
        source: InputSource,
        injected: bool,
        at_ms: u64,
    ) -> PhysicalInputProof {
        PhysicalInputProof {
            approval,
            nonce,
            source,
            injected,
            at_ms,
        }
    }
}
