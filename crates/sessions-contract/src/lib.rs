//! Kontrakt modułu `sessions` (docs/modules/sessions/SPEC.md, ADR 0006, ADR 0008).
//!
//! Sesja = osobna, szyfrowana baza SQLite (klucz w [`KeyVault`]). Historia rozmowy jest
//! **append-only** i ma postać drzewa: każda tura ma `parent` i `branch`; „edytuj” i „ponów”
//! tworzą nową gałąź przez [`SessionHistory::fork_from`], nigdy nie modyfikują wcześniejszych tur.
//! Widoczna rozmowa to projekcja gałęzi ([`SessionHistory::branch_projection`]).
//!
//! Kontrakty są synchroniczne (SQLite blokuje); wywołujący z kodu async używa `spawn_blocking`.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

mod api;
mod error;
mod ids;
mod import;
mod naming;
mod session;
mod turn;
mod vault;

#[cfg(feature = "contract-tests")]
pub mod contract_tests;

pub use api::{
    DeleteReport, SessionCatalog, SessionDbProvider, SessionHistory, Sessions, Siblings,
};
pub use error::SessionError;
pub use ids::{AgentId, BranchId, ProjectId, SessionId, TurnId};
pub use import::{MAX_PORTABLE_ID_LEN, PortableSession, TreeCursor, is_portable_session_id};
pub use naming::{DEFAULT_TITLE, session_dir_name, title_key, unique_dir_name};
pub use session::{
    NewSession, PrivacyTag, SessionMeta, SessionPatch, SessionQuery, SessionSort, SessionSummary,
    SessionTemplate, apply_query, normalize_tags,
};
pub use turn::{
    AttachmentRef, Author, Block, HeardPrefix, ModelUsage, NewTurn, Role, Turn, TurnContent,
    validate_heard_prefix, validate_new_turn,
};
pub use vault::{INDEX_KEY_NAME, KeyVault, VaultError, load_or_create_key, session_key_name};

/// Re-eksport klucza bazy i uchwytu bazy z `lib-sqlstore` (typy używane w kontrakcie).
pub use lib_sqlstore::{Db, DbKey};

/// Nazwy zdarzeń modułu (konwencja `<moduł>.<obiekt>.<czynność>`). Ładunki zawierają wyłącznie
/// identyfikatory i liczniki — nigdy treści tur (prywatność, PLAN §13).
pub mod events {
    /// Utworzono sesję (`{ "session": id }`).
    pub const SESSION_CREATED: &str = "session.created";
    /// Zmieniono metadane sesji (`{ "session": id }`).
    pub const SESSION_UPDATED: &str = "session.updated";
    /// Dopisano turę (`{ "session", "turn", "parent", "branch", "role" }`).
    pub const TURN_APPENDED: &str = "session.turn.appended";
    /// Powstała nowa gałąź (`{ "session", "branch", "turn" }`).
    pub const BRANCH_CREATED: &str = "session.branch.created";
    /// Sesja oznaczona jako skażona treścią niezaufaną (`{ "session" }`).
    pub const SESSION_TAINTED: &str = "session.tainted";
    /// Sesja zarchiwizowana lub przywrócona z archiwum (`{ "session", "archived" }`).
    pub const SESSION_ARCHIVED: &str = "session.archived";
    /// Sesja przeniesiona do kosza logicznego (`{ "session" }`).
    pub const SESSION_TRASHED: &str = "session.trashed";
    /// Sesja przywrócona z kosza (`{ "session" }`).
    pub const SESSION_RESTORED: &str = "session.restored";
    /// Sesja usunięta ostatecznie — crypto-shredding (`{ "session" }`).
    pub const SESSION_DELETED: &str = "session.deleted";
    /// Zaimportowano partię tur (`{ "session", "count" }`) — jedno zdarzenie zamiast `count` razy
    /// `session.turn.appended` (import `.alfa`).
    pub const TURNS_IMPORTED: &str = "session.turns.imported";
}
