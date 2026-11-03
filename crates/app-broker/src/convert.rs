//! Przekłady między typami Sędziego Cieni a DTO poleceń (`app-api`): poziom autonomii, zasięg
//! „Max dostęp” (plan 50 §8.1), wynik intencji i błąd Brokera jako błąd komendy.

use app_api::dto::{
    AutonomyLevel, BrokerIntentResult, BrokerIntentStatus, ChildLockRequest, ChildLockState,
    MaxAccess, SecretInput,
};
use app_api::error::{AppError, ErrorCode};
use risk_classifier_contract::AutonomyLevel as Level;
use safety_broker_contract::{
    BrokerError, ChildLockChange, ChildLockView, ModelScope, OPEN_UNTIL_CLOSE, Pin,
};

/// Poziom z polecenia → poziom Sędziego.
pub fn to_level(level: AutonomyLevel) -> Level {
    match level {
        AutonomyLevel::L0 => Level::L0,
        AutonomyLevel::L1 => Level::L1,
        AutonomyLevel::L2 => Level::L2,
        AutonomyLevel::L3 => Level::L3,
        AutonomyLevel::L4 => Level::L4,
    }
}

/// Poziom Sędziego → poziom w odpowiedzi.
pub fn from_level(level: Level) -> AutonomyLevel {
    match level {
        Level::L0 => AutonomyLevel::L0,
        Level::L1 => AutonomyLevel::L1,
        Level::L2 => AutonomyLevel::L2,
        Level::L3 => AutonomyLevel::L3,
        Level::L4 => AutonomyLevel::L4,
    }
}

/// Zasięg „Max dostęp” z polecenia → zasięg Sędziego.
pub fn to_scope(scope: MaxAccess) -> ModelScope {
    match scope {
        MaxAccess::Local => ModelScope::Local,
        MaxAccess::Cloud => ModelScope::Cloud,
        MaxAccess::All => ModelScope::All,
    }
}

/// Zasięg Sędziego → zasięg w odpowiedzi.
pub fn from_scope(scope: ModelScope) -> MaxAccess {
    match scope {
        ModelScope::Local => MaxAccess::Local,
        ModelScope::Cloud => MaxAccess::Cloud,
        ModelScope::All => MaxAccess::All,
    }
}

/// Zmiana przyjęta od razu (obniżenie).
pub fn applied() -> BrokerIntentResult {
    BrokerIntentResult {
        status: BrokerIntentStatus::Applied,
        request_id: String::new(),
    }
}

/// Zmiana profilu Dziecko z polecenia → zmiana dla Sędziego.
pub fn to_child(change: ChildLockRequest) -> Result<ChildLockChange, AppError> {
    let pin = |s: &SecretInput| Pin::new(s.expose()).map_err(AppError::invalid);
    Ok(match change {
        ChildLockRequest::SetPin { old, new } => ChildLockChange::SetPin {
            old: old.as_ref().map(pin).transpose()?,
            new: pin(&new)?,
        },
        ChildLockRequest::Enable { pin: p, on } => ChildLockChange::Enable { pin: pin(&p)?, on },
        ChildLockRequest::Open { pin: p, minutes } => ChildLockChange::Open {
            pin: pin(&p)?,
            for_ms: minutes.map(|m| u64::from(m.max(1)) * 60_000),
        },
        ChildLockRequest::Close => ChildLockChange::Close,
    })
}

/// Stan profilu Dziecko z Sędziego → odpowiedź (termin „do zamknięcia Alfy” bez liczby).
pub fn from_child(v: ChildLockView) -> ChildLockState {
    ChildLockState {
        pin_set: v.pin_set,
        enabled: v.enabled,
        open: v.open_until_ms.is_some(),
        open_until_ms: v.open_until_ms.filter(|u| *u != OPEN_UNTIL_CLOSE),
        blocked_until_ms: v.blocked_until_ms,
        attempts_left: v.attempts_left,
    }
}

/// Błąd Brokera → błąd komendy.
pub fn broker_error(e: BrokerError) -> AppError {
    let code = match &e {
        BrokerError::KernelBlock(_) | BrokerError::Unauthorized(_) => ErrorCode::Forbidden,
        BrokerError::AuditUnavailable(_) => ErrorCode::Unavailable,
        BrokerError::UnknownApproval(_) | BrokerError::UnknownSession(_) => ErrorCode::NotFound,
        _ => ErrorCode::InvalidInput,
    };
    AppError::new(code, format!("Sędzia Cieni: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn conversions_round_trip() {
        use AutonomyLevel::*;
        for l in [L0, L1, L2, L3, L4] {
            assert_eq!(from_level(to_level(l)), l);
        }
        for s in [MaxAccess::Local, MaxAccess::Cloud, MaxAccess::All] {
            assert_eq!(from_scope(to_scope(s)), s);
        }
        assert_eq!(applied().status, BrokerIntentStatus::Applied);
    }
}
