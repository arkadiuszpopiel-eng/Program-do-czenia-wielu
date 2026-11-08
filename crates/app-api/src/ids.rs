//! Identyfikatory DTO: komendy `turns_rate/…` i `files_preview/…` dostają samo id tury/pliku,
//! więc id niesie sesję: tura `"<sesja>:t<n>"`, artefakt `"<sesja>:a<id>"`, zdarzenie osi
//! czasu `"<sesja>:e<n>"`. Identyfikatory sesji (UUIDv7) nie zawierają `:`.

use artifacts_contract::ArtifactId;
use sessions_contract::{SessionId, TurnId};

use crate::error::AppError;

/// Id tury w DTO.
pub fn turn_dto(session: &SessionId, turn: TurnId) -> String {
    format!("{session}:t{}", turn.0)
}

/// Parsuje id tury z DTO.
pub fn parse_turn(id: &str) -> Result<(SessionId, TurnId), AppError> {
    let bad = || AppError::invalid(format!("Nieprawidłowy identyfikator tury „{id}”."));
    let (session, rest) = id.split_once(':').ok_or_else(bad)?;
    let n = rest
        .strip_prefix('t')
        .and_then(|n| n.parse::<u64>().ok())
        .filter(|n| *n > 0)
        .ok_or_else(bad)?;
    if session.is_empty() {
        return Err(bad());
    }
    Ok((SessionId::new(session), TurnId(n)))
}

/// Parsuje id tury, która musi należeć do sesji `session`.
pub fn parse_turn_in(session: &SessionId, id: &str) -> Result<TurnId, AppError> {
    let (owner, turn) = parse_turn(id)?;
    if &owner != session {
        return Err(AppError::invalid(format!(
            "Tura „{id}” nie należy do sesji „{session}”."
        )));
    }
    Ok(turn)
}

/// Id artefaktu w DTO.
pub fn artifact_dto(session: &SessionId, artifact: &ArtifactId) -> String {
    format!("{session}:a{artifact}")
}

/// Parsuje id artefaktu z DTO.
pub fn parse_artifact(id: &str) -> Result<(SessionId, ArtifactId), AppError> {
    let bad = || AppError::invalid(format!("Nieprawidłowy identyfikator pliku „{id}”."));
    let (session, rest) = id.split_once(':').ok_or_else(bad)?;
    let artifact = rest
        .strip_prefix('a')
        .filter(|a| !a.is_empty())
        .ok_or_else(bad)?;
    if session.is_empty() {
        return Err(bad());
    }
    Ok((SessionId::new(session), ArtifactId(artifact.to_owned())))
}

/// Nowe id zdarzenia osi czasu w DTO.
pub fn timeline_dto(session: &SessionId) -> String {
    format!("{session}:e{}", uuid::Uuid::new_v4().simple())
}

/// Token cofnięcia kroku dziennika `undo-journal` w DTO (`ToolStep.undo_token`): `"<sesja>:u<krok>"`.
pub fn undo_dto(session: &SessionId, step: u64) -> String {
    format!("{session}:u{step}")
}

/// Parsuje token cofnięcia z DTO.
pub fn parse_undo(token: &str) -> Result<(SessionId, u64), AppError> {
    let bad = || AppError::invalid(format!("Nieprawidłowy token cofnięcia „{token}”."));
    let (session_id, rest) = token.split_once(':').ok_or_else(bad)?;
    let step = rest
        .strip_prefix('u')
        .and_then(|n| n.parse::<u64>().ok())
        .ok_or_else(bad)?;
    Ok((session(session_id)?, step))
}

/// Token cofnięcia zapisu schowka (`tools-clipboard`) w DTO: `"<sesja>:c<id>"`.
pub fn undo_clip_dto(session: &SessionId, id: u64) -> String {
    format!("{session}:c{id}")
}

/// Token cofnięcia zapisu zmiennej użytkownika (`tools-system`, `system_env_set`) w DTO:
/// `"<sesja>:v<krok>"`.
pub fn undo_env_dto(session: &SessionId, id: u64) -> String {
    format!("{session}:v{id}")
}

/// Rodzaj tokenu cofnięcia.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum UndoKind {
    /// Krok dziennika `undo-journal` (`fs.*`, snapshot shella).
    Journal,
    /// Zapis schowka.
    Clipboard,
    /// Zapis zmiennej użytkownika (`tools-system`).
    System,
}

/// Parsuje token cofnięcia dowolnej usługi (`u` — dziennik, `c` — schowek, `v` — zmienna).
pub fn parse_any_undo(token: &str) -> Result<(SessionId, UndoKind, u64), AppError> {
    let bad = || AppError::invalid(format!("Nieprawidłowy token cofnięcia „{token}”."));
    let (session_id, rest) = token.split_once(':').ok_or_else(bad)?;
    let (kind, number) = match rest.split_at_checked(1) {
        Some(("u", n)) => (UndoKind::Journal, n),
        Some(("c", n)) => (UndoKind::Clipboard, n),
        Some(("v", n)) => (UndoKind::System, n),
        _ => return Err(bad()),
    };
    let id = number.parse::<u64>().map_err(|_| bad())?;
    Ok((session(session_id)?, kind, id))
}

/// Id przebiegu agentki w DTO: `"<sesja>:r<przebieg>"`.
pub fn run_dto(session: &SessionId, run: &str) -> String {
    format!("{session}:r{run}")
}

/// Id kroku przebiegu w DTO: `"<sesja>:r<przebieg>:s<n>"`.
pub fn step_dto(session: &SessionId, run: &str, step: u32) -> String {
    format!("{session}:r{run}:s{step}")
}

/// Parsuje id kroku przebiegu → (sesja, przebieg, numer).
pub fn parse_step(id: &str) -> Result<(SessionId, String, u32), AppError> {
    let bad = || AppError::invalid(format!("Nieprawidłowy identyfikator kroku „{id}”."));
    let (session_id, rest) = id.split_once(':').ok_or_else(bad)?;
    let (run, step) = rest.rsplit_once(':').ok_or_else(bad)?;
    let run = run
        .strip_prefix('r')
        .filter(|r| !r.is_empty())
        .ok_or_else(bad)?;
    let n = step
        .strip_prefix('s')
        .and_then(|n| n.parse::<u32>().ok())
        .ok_or_else(bad)?;
    if run.contains(':') {
        return Err(bad());
    }
    Ok((session(session_id)?, run.to_owned(), n))
}

/// Waliduje identyfikator sesji z UI (niepusty, bez `:` i znaków sterujących).
pub fn session(id: &str) -> Result<SessionId, AppError> {
    if id.is_empty() || id.len() > 128 || id.contains(':') || id.chars().any(char::is_control) {
        return Err(AppError::invalid(format!(
            "Nieprawidłowy identyfikator sesji „{id}”."
        )));
    }
    Ok(SessionId::new(id))
}
