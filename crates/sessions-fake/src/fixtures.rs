//! Fixture'y rozmów (działają na dowolnej implementacji [`Sessions`]).

use sessions_contract::{
    HeardPrefix, NewSession, NewTurn, SessionError, SessionId, Sessions, Turn,
};

/// Rozmowa z gałęziami i barge-in:
///
/// ```text
/// u1 „Opowiedz o pogodzie”
/// ├── a1 „Dziś będzie słonecznie, a wieczorem…” (usłyszane: „Dziś będzie słonecznie”, przybliżone)
/// │   └── u2 „Stop, a jutro?”
/// │       └── a2 „Jutro deszcz.”
/// └── a1b „Pogoda: 20°C” (ponów innym modelem — gałąź 2)
/// ```
#[derive(Debug, Clone)]
pub struct BargeInConversation {
    /// Sesja.
    pub session: SessionId,
    /// Pierwsza wiadomość użytkownika.
    pub u1: Turn,
    /// Odpowiedź przerwana (z usłyszanym prefiksem).
    pub a1: Turn,
    /// Przerwanie użytkownika.
    pub u2: Turn,
    /// Odpowiedź po przerwaniu.
    pub a2: Turn,
    /// Wariant odpowiedzi a1 („ponów”).
    pub a1b: Turn,
}

/// Tekst przerwanej odpowiedzi (`assistant_full`).
pub const INTERRUPTED_FULL: &str = "Dziś będzie słonecznie, a wieczorem przyjdzie burza z gradem.";

/// Liczba znaków usłyszanych przed przerwaniem.
pub const HEARD_CHARS: usize = 22;

/// Buduje [`BargeInConversation`] w nowej sesji.
pub fn barge_in_conversation(s: &dyn Sessions) -> Result<BargeInConversation, SessionError> {
    let session = s
        .create_session(NewSession {
            title: "Pogoda (barge-in)".into(),
            ..NewSession::default()
        })?
        .id;
    let u1 = s.append_turn(&session, None, NewTurn::user("Opowiedz o pogodzie"))?;
    let mut interrupted = NewTurn::assistant("alfa", INTERRUPTED_FULL);
    interrupted.heard_prefix = Some(HeardPrefix {
        chars: HEARD_CHARS,
        approximate: true,
    });
    let a1 = s.append_turn(&session, Some(u1.id), interrupted)?;
    let u2 = s.append_turn(&session, Some(a1.id), NewTurn::user("Stop, a jutro?"))?;
    let a2 = s.append_turn(
        &session,
        Some(u2.id),
        NewTurn::assistant("alfa", "Jutro deszcz."),
    )?;
    let a1b = s.fork_from(&session, a1.id, NewTurn::assistant("beta", "Pogoda: 20°C"))?;
    s.set_active_leaf(&session, a2.id)?;
    Ok(BargeInConversation {
        session,
        u1,
        a1,
        u2,
        a2,
        a1b,
    })
}

/// Liniowa rozmowa `n` tur (na przemian użytkownik/asystentka) — do pomiarów wydajności.
pub fn linear_conversation(
    s: &dyn Sessions,
    n: usize,
) -> Result<(SessionId, Vec<Turn>), SessionError> {
    let session = s
        .create_session(NewSession {
            title: format!("Rozmowa {n} tur"),
            ..NewSession::default()
        })?
        .id;
    let mut turns: Vec<Turn> = Vec::with_capacity(n);
    for i in 0..n {
        let parent = turns.last().map(|t| t.id);
        let new = if i % 2 == 0 {
            NewTurn::user(format!("Pytanie {i}: jak działa żółta łódź numer {i}?"))
        } else {
            NewTurn::assistant(
                "alfa",
                format!("Odpowiedź {i}: łódź pływa, bo wypiera wodę."),
            )
        };
        turns.push(s.append_turn(&session, parent, new)?);
    }
    Ok((session, turns))
}
