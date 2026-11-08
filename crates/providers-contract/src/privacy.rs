//! Tagi prywatności i jurysdykcji (PLAN §5.5). Egzekwuje Router; adapter sprawdza ponownie
//! (obrona w głąb) i odmawia **przed** jakimkolwiek ruchem sieciowym.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::error::{ProviderError, ProviderErrorKind};

/// Tag prywatności żądania (sesji).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum PrivacyTag {
    /// Zwykła sesja.
    #[default]
    Normal,
    /// Sesja „prywatne" — nigdy do tras CN / „może trenować" / nieznanych.
    Private,
}

/// Wymagania prywatności żądania.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RequestPrivacy {
    /// Tag sesji.
    #[serde(default)]
    pub tag: PrivacyTag,
    /// Dozwolone jurysdykcje (kody jak w katalogu, np. `EU`); pusta lista = bez ograniczeń.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub jurisdiction_allow: Vec<String>,
}

/// Profil prywatności dostawcy z katalogu (`privacy_tag`, `jurisdiction`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ProviderPrivacy {
    /// Tag z rejestru zgodności, np. `cn-may-train`, `eu`, `unknown`.
    pub tag: String,
    /// Jurysdykcja, np. `CN`, `SG|EU`, `unknown`.
    pub jurisdiction: String,
}

impl Default for ProviderPrivacy {
    /// Nieznany profil — traktowany ostrożnie (jak „może trenować").
    fn default() -> Self {
        Self {
            tag: "unknown".into(),
            jurisdiction: "unknown".into(),
        }
    }
}

impl ProviderPrivacy {
    /// Profil z wartości katalogu.
    pub fn new(tag: impl Into<String>, jurisdiction: impl Into<String>) -> Self {
        Self {
            tag: tag.into(),
            jurisdiction: jurisdiction.into(),
        }
    }

    /// Czy dostawca może obsłużyć sesję „prywatne": tag nie jest `unknown`, nie zawiera
    /// `may-train`, a jurysdykcja nie obejmuje `CN` (README katalogu: `unknown` = ostrożnie).
    pub fn allows_private(&self) -> bool {
        let tag = self.tag.to_ascii_lowercase();
        let cn = self
            .jurisdiction
            .split('|')
            .any(|j| j.eq_ignore_ascii_case("CN"));
        tag != "unknown" && !tag.contains("may-train") && !tag.starts_with("cn") && !cn
    }

    /// Czy jurysdykcja dostawcy mieści się w liście dozwolonych (pusta lista = tak).
    pub fn in_jurisdictions(&self, allow: &[String]) -> bool {
        allow.is_empty()
            || self
                .jurisdiction
                .split('|')
                .any(|j| allow.iter().any(|a| a.eq_ignore_ascii_case(j)))
    }
}

/// Sprawdza, czy żądanie może trafić do dostawcy. Błąd = `PrivacyBlocked` (bez fallbacku
/// na ten sam typ trasy — Router wybiera inną, zgodną trasę).
///
/// ```
/// use providers_contract::{check_privacy, PrivacyTag, ProviderPrivacy, RequestPrivacy};
/// let private = RequestPrivacy { tag: PrivacyTag::Private, jurisdiction_allow: vec![] };
/// assert!(check_privacy(&private, &ProviderPrivacy::new("cn-may-train", "CN")).is_err());
/// assert!(check_privacy(&private, &ProviderPrivacy::new("eu", "EU")).is_ok());
/// assert!(check_privacy(&RequestPrivacy::default(), &ProviderPrivacy::default()).is_ok());
/// ```
pub fn check_privacy(
    req: &RequestPrivacy,
    provider: &ProviderPrivacy,
) -> Result<(), ProviderError> {
    if req.tag == PrivacyTag::Private && !provider.allows_private() {
        return Err(ProviderError::new(
            ProviderErrorKind::PrivacyBlocked,
            format!(
                "sesja prywatna nie może trafić do dostawcy z tagiem `{}` / jurysdykcją `{}`",
                provider.tag, provider.jurisdiction
            ),
        ));
    }
    if !provider.in_jurisdictions(&req.jurisdiction_allow) {
        return Err(ProviderError::new(
            ProviderErrorKind::PrivacyBlocked,
            format!(
                "jurysdykcja dostawcy `{}` spoza dozwolonych {:?}",
                provider.jurisdiction, req.jurisdiction_allow
            ),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn private() -> RequestPrivacy {
        RequestPrivacy {
            tag: PrivacyTag::Private,
            jurisdiction_allow: vec![],
        }
    }

    #[test]
    fn private_blocks_may_train_cn_and_unknown() {
        for (tag, jur) in [
            ("cn-may-train", "CN"),
            ("google-personal-may-train", "unknown"),
            ("unknown", "US"),
            ("sg", "SG|CN"),
        ] {
            assert!(
                check_privacy(&private(), &ProviderPrivacy::new(tag, jur)).is_err(),
                "{tag}/{jur}"
            );
        }
        for (tag, jur) in [
            ("eu", "EU"),
            ("google-paid-eea-no-train", "unknown"),
            ("sg", "SG|EU"),
        ] {
            assert!(
                check_privacy(&private(), &ProviderPrivacy::new(tag, jur)).is_ok(),
                "{tag}/{jur}"
            );
        }
    }

    #[test]
    fn jurisdiction_allow_list() {
        let req = RequestPrivacy {
            tag: PrivacyTag::Normal,
            jurisdiction_allow: vec!["EU".into()],
        };
        assert!(check_privacy(&req, &ProviderPrivacy::new("sg", "SG|EU")).is_ok());
        let err = check_privacy(&req, &ProviderPrivacy::new("sg", "SG")).err();
        assert_eq!(err.map(|e| e.kind), Some(ProviderErrorKind::PrivacyBlocked));
    }
}
