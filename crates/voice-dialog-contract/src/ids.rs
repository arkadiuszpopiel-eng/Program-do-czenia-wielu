//! Identyfikatory wypowiedzi i tur (nadawane przez automat, monotoniczne).

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Identyfikator wypowiedzi agentki (każde wznowienie/powtórzenie = nowa wypowiedź).
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    PartialOrd,
    Ord,
    Default,
    Serialize,
    Deserialize,
    JsonSchema,
)]
#[serde(transparent)]
pub struct UtteranceId(pub u64);

/// Identyfikator tury użytkownika przekazanej do LLM.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    PartialOrd,
    Ord,
    Default,
    Serialize,
    Deserialize,
    JsonSchema,
)]
#[serde(transparent)]
pub struct TurnId(pub u64);
