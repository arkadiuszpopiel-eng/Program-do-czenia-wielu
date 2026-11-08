//! Decyzja „czy wolno użyć trasy w tej sesji” (`route_allowed`) — wspólna reguła `-impl` i `-fake`.

use std::collections::BTreeSet;
use std::fmt;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::status::RouteStatus;
use crate::table::RouteView;
use crate::tags::{PrivacyTag, SessionTag, TrainingRisk};

/// Polityka prywatności dla sesji „prywatne” (dane Jądra).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct PrivacyPolicy {
    /// Tag `unknown` (lub brak tagów) blokuje sesję prywatną (zasada ostrożności, domyślnie tak).
    pub unknown_privacy_blocks_private: bool,
    /// Nieznana jurysdykcja blokuje sesję prywatną (domyślnie nie — decyduje tag prywatności).
    pub unknown_jurisdiction_blocks_private: bool,
    /// Jurysdykcje zakazane dla sesji prywatnych (domyślnie `CN`).
    pub blocked_jurisdictions: BTreeSet<String>,
}

impl Default for PrivacyPolicy {
    fn default() -> Self {
        Self {
            unknown_privacy_blocks_private: true,
            unknown_jurisdiction_blocks_private: false,
            blocked_jurisdictions: BTreeSet::from(["CN".to_owned()]),
        }
    }
}

/// Powód decyzji (stabilny kod dla UI i audytu).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "code", rename_all = "snake_case")]
pub enum DecisionReason {
    /// Trasa zielona, włączona, zgodna z polityką sesji.
    Allowed,
    /// Trasa szara włączona przez użytkownika albo trasa API z niezweryfikowanego wpisu — z ostrzeżeniem.
    AllowedWithWarning {
        /// Degradacja z powodu nieświeżego rejestru.
        stale: bool,
        /// Wpis katalogu niezweryfikowany.
        unverified: bool,
    },
    /// Nieznana trasa (brak w rejestrze i w katalogu).
    UnknownRoute,
    /// Trasa zabroniona.
    Forbidden,
    /// Trasa wyłączona (wyłącznik albo domyślnie wyłączona szara/nieświeża).
    Disabled {
        /// Czy wyłączona przez degradację nieświeżego wpisu.
        stale: bool,
    },
    /// Sesja prywatna, trasa w zakazanej jurysdykcji.
    PrivateJurisdiction {
        /// Kod jurysdykcji.
        jurisdiction: String,
    },
    /// Sesja prywatna, dostawca może trenować na danych.
    PrivateMayTrain {
        /// Tag, który to powoduje.
        tag: PrivacyTag,
    },
    /// Sesja prywatna, tag prywatności nieznany.
    PrivateUnknownPrivacy,
    /// Sesja prywatna, jurysdykcja nieznana (gdy polityka tego wymaga).
    PrivateUnknownJurisdiction,
}

impl fmt::Display for DecisionReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DecisionReason::Allowed => f.write_str("trasa dozwolona"),
            DecisionReason::AllowedWithWarning { stale, unverified } => {
                f.write_str("trasa szara — dozwolona z ostrzeżeniem")?;
                if *stale {
                    f.write_str(" (rejestr nieświeży, wymaga ponownej weryfikacji)")?;
                }
                if *unverified {
                    f.write_str(" (wpis katalogu niezweryfikowany)")?;
                }
                Ok(())
            }
            DecisionReason::UnknownRoute => f.write_str("nieznana trasa"),
            DecisionReason::Forbidden => f.write_str("trasa zabroniona przez rejestr zgodności"),
            DecisionReason::Disabled { stale: true } => {
                f.write_str("trasa wyłączona: rejestr nieświeży, wymaga ponownej weryfikacji")
            }
            DecisionReason::Disabled { stale: false } => f.write_str("trasa wyłączona"),
            DecisionReason::PrivateJurisdiction { jurisdiction } => {
                write!(f, "sesja prywatna: jurysdykcja {jurisdiction} niedozwolona")
            }
            DecisionReason::PrivateMayTrain { tag } => {
                write!(
                    f,
                    "sesja prywatna: dostawca może trenować na danych ({tag})"
                )
            }
            DecisionReason::PrivateUnknownPrivacy => f.write_str(
                "sesja prywatna: tag prywatności nieznany (traktowany jak „może trenować”)",
            ),
            DecisionReason::PrivateUnknownJurisdiction => {
                f.write_str("sesja prywatna: jurysdykcja nieznana")
            }
        }
    }
}

/// Decyzja `route_allowed`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Decision {
    /// Czy wolno użyć trasy.
    pub allowed: bool,
    /// Powód (także przy zgodzie — np. ostrzeżenie o szarej trasie).
    pub reason: DecisionReason,
}

impl Decision {
    fn deny(reason: DecisionReason) -> Self {
        Self {
            allowed: false,
            reason,
        }
    }

    fn allow(reason: DecisionReason) -> Self {
        Self {
            allowed: true,
            reason,
        }
    }
}

/// Reguła decyzji. Kolejność: istnienie → zabroniona → wyłącznik → polityka sesji → ostrzeżenie.
pub fn decide(view: Option<&RouteView>, session: SessionTag, policy: &PrivacyPolicy) -> Decision {
    let Some(view) = view else {
        return Decision::deny(DecisionReason::UnknownRoute);
    };
    if view.effective.status == RouteStatus::Forbidden {
        return Decision::deny(DecisionReason::Forbidden);
    }
    if !view.enabled {
        return Decision::deny(DecisionReason::Disabled {
            stale: view.effective.stale,
        });
    }
    if session == SessionTag::Private
        && let Some(reason) = private_violation(view, policy)
    {
        return Decision::deny(reason);
    }
    if view.effective.status == RouteStatus::Grey {
        return Decision::allow(DecisionReason::AllowedWithWarning {
            stale: view.effective.stale,
            unverified: view.effective.unverified,
        });
    }
    Decision::allow(DecisionReason::Allowed)
}

fn private_violation(view: &RouteView, policy: &PrivacyPolicy) -> Option<DecisionReason> {
    let tags = &view.tags;
    if let Some(code) = tags
        .jurisdiction
        .codes()
        .find(|c| policy.blocked_jurisdictions.contains(*c))
    {
        return Some(DecisionReason::PrivateJurisdiction {
            jurisdiction: code.to_owned(),
        });
    }
    if let Some(tag) = tags
        .privacy
        .iter()
        .find(|t| t.training_risk() == TrainingRisk::MayTrain)
    {
        return Some(DecisionReason::PrivateMayTrain { tag: *tag });
    }
    if policy.unknown_privacy_blocks_private && tags.training_risk() == TrainingRisk::Unknown {
        return Some(DecisionReason::PrivateUnknownPrivacy);
    }
    if policy.unknown_jurisdiction_blocks_private && tags.jurisdiction.is_unknown() {
        return Some(DecisionReason::PrivateUnknownJurisdiction);
    }
    None
}
