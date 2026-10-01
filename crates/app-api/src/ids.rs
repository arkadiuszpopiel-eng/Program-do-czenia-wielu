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

/// Waliduje identyfikator sesji z UI (niepusty, bez `:` i znaków sterujących).
pub fn session(id: &str) -> Result<SessionId, AppError> {
    if id.is_empty() || id.len() > 128 || id.contains(':') || id.chars().any(char::is_control) {
        return Err(AppError::invalid(format!(
            "Nieprawidłowy identyfikator sesji „{id}”."
        )));
    }
    Ok(SessionId::new(id))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn turn_ids_roundtrip_and_reject_garbage() {
        let s = SessionId::new("0192-abc");
        let id = turn_dto(&s, TurnId(12));
        assert_eq!(id, "0192-abc:t12");
        assert_eq!(parse_turn(&id).unwrap(), (s.clone(), TurnId(12)));
        assert_eq!(parse_turn_in(&s, &id).unwrap(), TurnId(12));
        assert!(parse_turn_in(&SessionId::new("inna"), &id).is_err());
        for bad in ["", "q4b", ":t1", "s:x1", "s:t0", "s:t-1", "s:t"] {
            assert!(parse_turn(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn artifact_ids_and_sessions() {
        let s = SessionId::new("s1");
        let id = artifact_dto(&s, &ArtifactId("a-7:x".into()));
        assert_eq!(parse_artifact(&id).unwrap().1, ArtifactId("a-7:x".into()));
        assert!(parse_artifact("s1:b").is_err());
        assert!(session("a:b").is_err());
        assert!(session("").is_err());
        assert_eq!(session("s-q3").unwrap(), SessionId::new("s-q3"));
        assert!(timeline_dto(&s).starts_with("s1:e"));
    }
}
