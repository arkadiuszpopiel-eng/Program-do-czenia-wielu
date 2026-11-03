//! Przekłady między typami Sędziego Cieni a DTO poleceń (`app-api`): poziom autonomii, zasięg
//! „Max dostęp” (plan 50 §8.1), wynik intencji i błąd Brokera jako błąd komendy.

use app_api::dto::{AutonomyLevel, BrokerIntentResult, BrokerIntentStatus, MaxAccess};
use app_api::error::{AppError, ErrorCode};
use risk_classifier_contract::AutonomyLevel as Level;
use safety_broker_contract::{BrokerError, ModelScope};

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
