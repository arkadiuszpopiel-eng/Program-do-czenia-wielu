//! „Max dostęp” (decyzja właściciela 3.11, plan 50 §8.1): przełącznik z zasięgiem modele lokalne /
//! w chmurze / wszystkie. Włączenie to prośba do Sędziego Cieni (karta w jego oknie), wyłączenie —
//! obniżenie bieżącego zasięgu, od razu. Stan trzyma i zapamiętuje Sędzia.

use app_api::dto::{BrokerIntentResult, BrokerIntentStatus, MaxAccess};
use app_api::error::AppError;
use risk_classifier_contract::AutonomyLevel as Level;
use safety_broker_contract::{AutonomyChangeRequest, AutonomyTarget, ChangeOrigin, ModelScope};

use super::{InprocBroker, broker_error};

pub(super) fn to_scope(scope: MaxAccess) -> ModelScope {
    match scope {
        MaxAccess::Local => ModelScope::Local,
        MaxAccess::Cloud => ModelScope::Cloud,
        MaxAccess::All => ModelScope::All,
    }
}

pub(super) fn from_scope(scope: ModelScope) -> MaxAccess {
    match scope {
        ModelScope::Local => MaxAccess::Local,
        ModelScope::Cloud => MaxAccess::Cloud,
        ModelScope::All => MaxAccess::All,
    }
}

fn applied() -> BrokerIntentResult {
    BrokerIntentResult {
        status: BrokerIntentStatus::Applied,
        request_id: String::new(),
    }
}

impl InprocBroker {
    /// Włączony zasięg według Sędziego.
    pub(super) fn max_access(&self) -> Option<MaxAccess> {
        self.engine.metrics().max_access.map(from_scope)
    }

    /// Włączenie (karta Sędziego) albo wyłączenie (od razu) „Max dostęp”.
    pub(super) async fn change_max_access(
        &self,
        scope: Option<MaxAccess>,
    ) -> Result<BrokerIntentResult, AppError> {
        let (scope, level) = match scope {
            Some(s) => (to_scope(s), Level::L4),
            None => match self.engine.metrics().max_access {
                Some(on) => (on, Level::L3),
                None => return Ok(applied()),
            },
        };
        let request = AutonomyChangeRequest {
            target: AutonomyTarget::Models { scope },
            level,
            until_ms: None,
            origin: ChangeOrigin::UserInterface,
        };
        match self.engine.request_autonomy_change(request).await {
            Ok(None) => Ok(applied()),
            Ok(Some(approval)) => self.present(approval.0),
            Err(e) => Err(broker_error(e)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scope_conversion_round_trips() {
        for s in [MaxAccess::Local, MaxAccess::Cloud, MaxAccess::All] {
            assert_eq!(from_scope(to_scope(s)), s);
        }
    }
}
