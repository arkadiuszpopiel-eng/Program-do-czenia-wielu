//! Wpisy dziennika o zmianach ustawień bezpieczeństwa (plan 50 Z.7): tryb pracy, Max dostęp i każda operacja na
//! kłódce profilu Dziecko — także nieudane próby PIN-u. Tekst wpisu „audit” na osi czasu aktywnej rozmowy
//! (zapis: `app-core`). PIN nigdy nie trafia do wpisu.

use app_api::AppError;
use app_api::dto::{
    AutonomyLevel, BrokerIntentResult, BrokerIntentStatus, ChildLockRequest, EventLevel, MaxAccess,
};

/// Nazwa trybu pracy jak w menu (plan 50 §4.1).
pub fn mode_name(level: AutonomyLevel) -> &'static str {
    match level {
        AutonomyLevel::L0 => "Plan",
        AutonomyLevel::L1 => "Ręcznie",
        AutonomyLevel::L2 => "Akceptuj edycje",
        AutonomyLevel::L3 => "Automatycznie",
        AutonomyLevel::L4 => "Pełny dostęp",
    }
}

fn scope_name(scope: MaxAccess) -> &'static str {
    match scope {
        MaxAccess::Local => "modele lokalne",
        MaxAccess::Cloud => "modele w chmurze",
        MaxAccess::All => "wszystkie",
    }
}

/// Wpis po prośbie o tryb pracy.
pub fn level_note(
    level: AutonomyLevel,
    session: bool,
    result: &Result<BrokerIntentResult, AppError>,
) -> (EventLevel, String) {
    let scope = if session {
        "ta rozmowa"
    } else {
        "wszystkie rozmowy"
    };
    let name = mode_name(level);
    match result {
        Ok(r) if r.status == BrokerIntentStatus::Applied => (
            EventLevel::Info,
            format!("Tryb pracy zmieniony na „{name}” ({scope})"),
        ),
        Ok(_) => (
            EventLevel::Warn,
            format!("Prośba o tryb „{name}” ({scope}) — czeka w oknie Sędziego Cieni"),
        ),
        Err(e) => (
            EventLevel::Warn,
            format!("Zmiana trybu na „{name}” odrzucona: {}", e.message),
        ),
    }
}

/// Wpis po prośbie o Max dostęp.
pub fn max_access_note(
    scope: Option<MaxAccess>,
    result: &Result<BrokerIntentResult, AppError>,
) -> (EventLevel, String) {
    match (scope, result) {
        (None, Ok(_)) => (EventLevel::Info, "Max dostęp wyłączony".to_owned()),
        (Some(s), Ok(_)) => (
            EventLevel::Warn,
            format!(
                "Prośba o Max dostęp ({}) — czeka w oknie Sędziego Cieni",
                scope_name(s)
            ),
        ),
        (_, Err(e)) => (
            EventLevel::Warn,
            format!("Max dostęp — odmowa: {}", e.message),
        ),
    }
}

/// Opis operacji na kłódce (bez PIN-u) — liczony, zanim zmiana trafi do Sędziego Cieni.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChildLockAction {
    what: &'static str,
    verify: bool,
}

/// Rodzaj operacji na kłódce.
pub fn child_lock_action(change: &ChildLockRequest) -> ChildLockAction {
    let what = match change {
        ChildLockRequest::SetPin { old: None, .. } => "ustawienie PIN-u rodzica",
        ChildLockRequest::SetPin { .. } => "zmiana PIN-u rodzica",
        ChildLockRequest::Enable { on: true, .. } => "włączenie profilu Dziecko",
        ChildLockRequest::Enable { on: false, .. } => "wyłączenie profilu Dziecko",
        ChildLockRequest::Open {
            minutes: Some(_), ..
        } => "otwarcie kłódki na czas",
        ChildLockRequest::Open { minutes: None, .. } => "otwarcie kłódki do zamknięcia Alfy",
        ChildLockRequest::Close => "zamknięcie kłódki",
        ChildLockRequest::Verify { .. } => "otwarcie dziennika rodzica",
        ChildLockRequest::Transcripts { on: true, .. } => "włączenie pełnej treści rozmów",
        ChildLockRequest::Transcripts { on: false, .. } => "wyłączenie pełnej treści rozmów",
    };
    ChildLockAction {
        what,
        verify: matches!(change, ChildLockRequest::Verify { .. }),
    }
}

/// Wpis po operacji na kłódce. Udane sprawdzenie PIN-u nie jest zapisywane — otwarcie dziennika rodzica
/// widać w samym dzienniku; nieudane próby (każdej operacji) — zawsze.
pub fn child_lock_note(
    action: ChildLockAction,
    ok: Result<(), &AppError>,
) -> Option<(EventLevel, String)> {
    let what = action.what;
    match ok {
        Ok(()) if action.verify => None,
        Ok(()) => Some((EventLevel::Info, format!("Profil Dziecko: {what}"))),
        Err(e) => Some((
            EventLevel::Warn,
            format!("Profil Dziecko: nieudana próba — {what} ({})", e.message),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn secret(s: &str) -> app_api::dto::SecretInput {
        serde_json::from_value(serde_json::Value::String(s.into())).unwrap()
    }

    fn opened() -> Result<BrokerIntentResult, AppError> {
        Ok(BrokerIntentResult {
            status: BrokerIntentStatus::OpenedBroker,
            request_id: "1".into(),
        })
    }

    #[test]
    fn level_notes_name_the_mode_and_scope() {
        let applied = Ok(BrokerIntentResult {
            status: BrokerIntentStatus::Applied,
            request_id: "1".into(),
        });
        let (level, title) = level_note(AutonomyLevel::L1, false, &applied);
        assert_eq!(level, EventLevel::Info);
        assert_eq!(
            title,
            "Tryb pracy zmieniony na „Ręcznie” (wszystkie rozmowy)"
        );
        let (level, title) = level_note(AutonomyLevel::L4, true, &opened());
        assert_eq!(level, EventLevel::Warn);
        assert!(title.contains("„Pełny dostęp” (ta rozmowa) — czeka"));
    }

    #[test]
    fn max_access_notes() {
        assert_eq!(max_access_note(None, &opened()).1, "Max dostęp wyłączony");
        assert!(
            max_access_note(Some(MaxAccess::Local), &opened())
                .1
                .contains("modele lokalne")
        );
    }

    #[test]
    fn failed_pin_is_logged_without_the_pin() {
        let pin = secret("2468");
        let open = ChildLockRequest::Open {
            pin,
            minutes: Some(30),
        };
        let err = AppError::forbidden("zły PIN");
        let (level, title) = child_lock_note(child_lock_action(&open), Err(&err)).unwrap();
        assert_eq!(level, EventLevel::Warn);
        assert!(title.starts_with("Profil Dziecko: nieudana próba — otwarcie kłódki na czas"));
        assert!(!title.contains("2468"));
        let verify = ChildLockRequest::Verify {
            pin: secret("2468"),
        };
        assert!(child_lock_note(child_lock_action(&verify), Ok(())).is_none());
        assert!(child_lock_note(child_lock_action(&verify), Err(&err)).is_some());
        assert_eq!(
            child_lock_note(child_lock_action(&ChildLockRequest::Close), Ok(()))
                .unwrap()
                .1,
            "Profil Dziecko: zamknięcie kłódki"
        );
    }
}
