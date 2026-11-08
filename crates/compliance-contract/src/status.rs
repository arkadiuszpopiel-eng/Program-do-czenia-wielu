//! Identyfikator trasy, statusy i reguła świeżości (subscription-routes.md §7).

use std::fmt;

use chrono::NaiveDate;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Domyślna liczba dni, po których wpis rejestru jest nieświeży.
pub const DEFAULT_MAX_AGE_DAYS: u32 = 30;

/// Identyfikator trasy: segmenty kebab-case rozdzielone kropką, np. `claude-code-cli`,
/// `deepseek.api` (trasa API dostawcy z katalogu, zob. [`RouteId::api`]).
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, JsonSchema)]
#[serde(transparent)]
pub struct RouteId(String);

impl RouteId {
    /// Tworzy identyfikator po walidacji; `None`, gdy format jest zły.
    pub fn new(id: impl Into<String>) -> Option<Self> {
        let id = id.into();
        is_route_id(&id).then_some(Self(id))
    }

    /// Trasa API dostawcy z katalogu: `<provider>.api`.
    pub fn api(provider: &str) -> Option<Self> {
        Self::new(format!("{provider}.api"))
    }

    /// Widok tekstowy.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Dostawca, jeśli to trasa API (`<provider>.api`).
    pub fn api_provider(&self) -> Option<&str> {
        self.0.strip_suffix(".api")
    }
}

impl fmt::Display for RouteId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for RouteId {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(deserializer)?;
        Self::new(raw.clone())
            .ok_or_else(|| serde::de::Error::custom(format!("niepoprawny id trasy `{raw}`")))
    }
}

/// Czy tekst jest segmentem kebab-case: `[a-z0-9]+(-[a-z0-9]+)*`.
pub fn is_kebab(s: &str) -> bool {
    !s.is_empty()
        && s.split('-').all(|seg| {
            !seg.is_empty()
                && seg
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
        })
}

fn is_route_id(s: &str) -> bool {
    s.len() <= 128 && s.split('.').all(is_kebab)
}

/// Status trasy w rejestrze (zielona / szara / zabroniona).
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum RouteStatus {
    /// Wolno (po weryfikacji).
    Green,
    /// Do weryfikacji — domyślnie wyłączona, włączenie z ostrzeżeniem.
    #[serde(rename = "gray", alias = "grey")]
    Grey,
    /// Nie budujemy / nie używamy.
    Forbidden,
}

impl RouteStatus {
    /// Gorszy z dwóch statusów (Forbidden > Grey > Green).
    #[must_use]
    pub fn worst(self, other: RouteStatus) -> RouteStatus {
        self.max(other)
    }
}

/// Status API dostawcy w katalogu (`providers-catalog/*.toml`, pole `compliance_status`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ProviderApiStatus {
    /// Zweryfikowany, zielony.
    Green,
    /// Szary.
    #[serde(rename = "gray", alias = "grey")]
    Gray,
    /// Zabroniony.
    Forbidden,
    /// Niezweryfikowany (szkic katalogu).
    Unverified,
}

impl ProviderApiStatus {
    /// Status trasy API: `unverified` traktujemy jak „szarą” (dozwolona z ostrzeżeniem).
    pub fn route_status(self) -> RouteStatus {
        match self {
            ProviderApiStatus::Green => RouteStatus::Green,
            ProviderApiStatus::Gray | ProviderApiStatus::Unverified => RouteStatus::Grey,
            ProviderApiStatus::Forbidden => RouteStatus::Forbidden,
        }
    }
}

/// Status efektywny: po uwzględnieniu świeżości rejestru.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct EffectiveStatus {
    /// Status po degradacji.
    pub status: RouteStatus,
    /// Czy zdegradowano z powodu nieświeżego wpisu.
    pub stale: bool,
    /// Czy trasa pochodzi z niezweryfikowanego wpisu katalogu.
    pub unverified: bool,
}

/// Czy wpis jest nieświeży: `dziś − verified_at > max_age_days` (daty w przyszłości są świeże).
pub fn is_stale(verified_at: NaiveDate, today: NaiveDate, max_age_days: u32) -> bool {
    (today - verified_at).num_days() > i64::from(max_age_days)
}

/// Degradacja: nieświeża trasa ma status co najwyżej „szary”; „zabroniona” zostaje zabroniona.
pub fn effective_status(
    declared: RouteStatus,
    verified_at: Option<NaiveDate>,
    today: NaiveDate,
    max_age_days: u32,
) -> EffectiveStatus {
    let stale = verified_at.is_some_and(|v| is_stale(v, today, max_age_days));
    let status = if stale {
        declared.worst(RouteStatus::Grey)
    } else {
        declared
    };
    EffectiveStatus {
        status,
        stale: stale && declared == RouteStatus::Green,
        unverified: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(y: i32, m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, day).unwrap()
    }

    #[test]
    fn route_ids() {
        assert!(RouteId::new("claude-code-cli").is_some());
        assert_eq!(
            RouteId::api("deepseek").unwrap().api_provider(),
            Some("deepseek")
        );
        for bad in ["", "Claude", "a..b", "a-", "-a", "a b", "a/b"] {
            assert!(RouteId::new(bad).is_none(), "{bad}");
        }
    }

    #[test]
    fn stale_boundary() {
        let v = d(2026, 9, 1);
        assert!(!is_stale(v, d(2026, 10, 1), 30));
        assert!(is_stale(v, d(2026, 10, 2), 30));
        assert!(!is_stale(v, d(2026, 8, 1), 30));
    }

    #[test]
    fn degradation_rules() {
        let v = Some(d(2026, 1, 1));
        let today = d(2026, 9, 30);
        let g = effective_status(RouteStatus::Green, v, today, 30);
        assert_eq!(g.status, RouteStatus::Grey);
        assert!(g.stale);
        let f = effective_status(RouteStatus::Forbidden, v, today, 30);
        assert_eq!(f.status, RouteStatus::Forbidden);
        let fresh = effective_status(RouteStatus::Green, Some(today), today, 30);
        assert_eq!(fresh.status, RouteStatus::Green);
        assert!(!fresh.stale);
    }

    #[test]
    fn status_serde_accepts_grey_alias() {
        let s: RouteStatus = serde_json::from_str("\"grey\"").unwrap();
        assert_eq!(s, RouteStatus::Grey);
        assert_eq!(serde_json::to_string(&s).unwrap(), "\"gray\"");
    }
}
