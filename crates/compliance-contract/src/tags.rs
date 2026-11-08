//! Tagi prywatności i jurysdykcji (docs/compliance/subscription-routes.md §5, PLAN §5.5).

use std::collections::BTreeSet;
use std::fmt;
use std::str::FromStr;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Tag prywatności z rejestru zgodności i katalogu dostawców.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
pub enum PrivacyTag {
    /// Konto osobiste Google — dane mogą trenować modele.
    #[serde(rename = "google-personal-may-train")]
    GooglePersonalMayTrain,
    /// Klucz płatny Google / EOG — nie trenuje.
    #[serde(rename = "google-paid-eea-no-train")]
    GooglePaidEeaNoTrain,
    /// xAI API — retencja 30 dni.
    #[serde(rename = "xai-retention-30d")]
    XaiRetention30d,
    /// Dane w Chinach/Singapurze, może trenować.
    #[serde(rename = "cn-may-train")]
    CnMayTrain,
    /// Singapur.
    #[serde(rename = "sg")]
    Sg,
    /// UE (Frankfurt).
    #[serde(rename = "eu")]
    Eu,
    /// Plan nie potwierdza — do weryfikacji (traktowany ostrożnie).
    #[serde(rename = "unknown")]
    Unknown,
}

/// Ryzyko użycia danych do trenowania wynikające z tagu.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum TrainingRisk {
    /// Dostawca deklaruje brak trenowania (albo tag dotyczy tylko regionu/retencji).
    NoTrain,
    /// Dane mogą trenować modele.
    MayTrain,
    /// Nie wiadomo (zasada ostrożności: jak „może trenować”).
    Unknown,
}

impl PrivacyTag {
    /// Wszystkie tagi (kolejność stała).
    pub const ALL: [PrivacyTag; 7] = [
        PrivacyTag::GooglePersonalMayTrain,
        PrivacyTag::GooglePaidEeaNoTrain,
        PrivacyTag::XaiRetention30d,
        PrivacyTag::CnMayTrain,
        PrivacyTag::Sg,
        PrivacyTag::Eu,
        PrivacyTag::Unknown,
    ];

    /// Klasyfikacja ryzyka trenowania. `xai-retention-30d`, `sg`, `eu` opisują retencję/region,
    /// nie trenowanie — plan (§5.5) wymienia jako „może trenować” tylko konto osobiste Google i CN.
    pub fn training_risk(self) -> TrainingRisk {
        match self {
            PrivacyTag::GooglePersonalMayTrain | PrivacyTag::CnMayTrain => TrainingRisk::MayTrain,
            PrivacyTag::Unknown => TrainingRisk::Unknown,
            PrivacyTag::GooglePaidEeaNoTrain
            | PrivacyTag::XaiRetention30d
            | PrivacyTag::Sg
            | PrivacyTag::Eu => TrainingRisk::NoTrain,
        }
    }

    /// Nazwa tekstowa (jak w plikach danych).
    pub fn as_str(self) -> &'static str {
        match self {
            PrivacyTag::GooglePersonalMayTrain => "google-personal-may-train",
            PrivacyTag::GooglePaidEeaNoTrain => "google-paid-eea-no-train",
            PrivacyTag::XaiRetention30d => "xai-retention-30d",
            PrivacyTag::CnMayTrain => "cn-may-train",
            PrivacyTag::Sg => "sg",
            PrivacyTag::Eu => "eu",
            PrivacyTag::Unknown => "unknown",
        }
    }
}

impl fmt::Display for PrivacyTag {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for PrivacyTag {
    type Err = TagError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::ALL
            .into_iter()
            .find(|t| t.as_str() == s)
            .ok_or_else(|| TagError::UnknownPrivacyTag(s.to_owned()))
    }
}

/// Błędy parsowania tagów.
#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq)]
pub enum TagError {
    /// Tag spoza listy.
    #[error("nieznany tag prywatności `{0}`")]
    UnknownPrivacyTag(String),
    /// Jurysdykcja w złym formacie (oczekiwano `XX`, `XX|YY` albo `unknown`).
    #[error("niepoprawna jurysdykcja `{0}` (oczekiwano `XX`, `XX|YY` albo `unknown`)")]
    InvalidJurisdiction(String),
}

/// Jurysdykcja: zbiór dwuliterowych kodów (np. `CN`, `SG|EU`); pusty zbiór = `unknown`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Jurisdiction(BTreeSet<String>);

impl Jurisdiction {
    /// Jurysdykcja nieznana.
    pub fn unknown() -> Self {
        Self::default()
    }

    /// Czy nieznana (brak kodów).
    pub fn is_unknown(&self) -> bool {
        self.0.is_empty()
    }

    /// Czy obejmuje kod (wielkie litery, np. `CN`).
    pub fn contains(&self, code: &str) -> bool {
        self.0.contains(code)
    }

    /// Kody w porządku alfabetycznym.
    pub fn codes(&self) -> impl Iterator<Item = &str> {
        self.0.iter().map(String::as_str)
    }

    /// Suma zbiorów (łączenie źródeł: rejestr + katalog).
    #[must_use]
    pub fn union(&self, other: &Jurisdiction) -> Jurisdiction {
        Jurisdiction(self.0.union(&other.0).cloned().collect())
    }
}

impl FromStr for Jurisdiction {
    type Err = TagError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s == "unknown" {
            return Ok(Self::unknown());
        }
        let invalid = || TagError::InvalidJurisdiction(s.to_owned());
        let mut codes = BTreeSet::new();
        for code in s.split('|') {
            let ok = code.len() == 2 && code.chars().all(|c| c.is_ascii_uppercase());
            if !ok {
                return Err(invalid());
            }
            codes.insert(code.to_owned());
        }
        Ok(Self(codes))
    }
}

impl fmt::Display for Jurisdiction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_unknown() {
            return f.write_str("unknown");
        }
        let joined: Vec<&str> = self.codes().collect();
        f.write_str(&joined.join("|"))
    }
}

impl Serialize for Jurisdiction {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for Jurisdiction {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(deserializer)?;
        raw.parse().map_err(serde::de::Error::custom)
    }
}

impl JsonSchema for Jurisdiction {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "Jurisdiction".into()
    }

    fn json_schema(_gen: &mut schemars::SchemaGenerator) -> schemars::Schema {
        schemars::json_schema!({
            "type": "string",
            "pattern": "^([A-Z]{2}(\\|[A-Z]{2})*|unknown)$",
            "description": "Kody jurysdykcji rozdzielone `|` albo `unknown`."
        })
    }
}

/// Tag sesji używany przez politykę prywatności (PLAN §5.5).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SessionTag {
    /// Zwykła sesja.
    #[default]
    Standard,
    /// Sesja „prywatne”: bez tras CN i „może trenować”.
    Private,
}

/// Tagi trasy: prywatność (zbiór) + jurysdykcja.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RouteTags {
    /// Tagi prywatności (suma ze wszystkich źródeł).
    pub privacy: BTreeSet<PrivacyTag>,
    /// Jurysdykcja (suma kodów ze wszystkich źródeł).
    pub jurisdiction: Jurisdiction,
}

impl RouteTags {
    /// Suma tagów (ostrożnie: łączenie nigdy nie usuwa ryzyka).
    #[must_use]
    pub fn union(&self, other: &RouteTags) -> RouteTags {
        RouteTags {
            privacy: self.privacy.union(&other.privacy).copied().collect(),
            jurisdiction: self.jurisdiction.union(&other.jurisdiction),
        }
    }

    /// Najgorsze ryzyko trenowania; brak tagów = `Unknown`.
    pub fn training_risk(&self) -> TrainingRisk {
        let risks: Vec<TrainingRisk> = self.privacy.iter().map(|t| t.training_risk()).collect();
        if risks.contains(&TrainingRisk::MayTrain) {
            TrainingRisk::MayTrain
        } else if risks.is_empty() || risks.contains(&TrainingRisk::Unknown) {
            TrainingRisk::Unknown
        } else {
            TrainingRisk::NoTrain
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn privacy_tags_round_trip() {
        for tag in PrivacyTag::ALL {
            assert_eq!(tag.as_str().parse::<PrivacyTag>().unwrap(), tag);
            let json = serde_json::to_string(&tag).unwrap();
            assert_eq!(json, format!("\"{}\"", tag.as_str()));
        }
        assert!("public".parse::<PrivacyTag>().is_err());
    }

    #[test]
    fn jurisdiction_parse_and_union() {
        let a: Jurisdiction = "SG|EU".parse().unwrap();
        assert!(a.contains("SG") && a.contains("EU"));
        assert_eq!(a.to_string(), "EU|SG");
        let u: Jurisdiction = "unknown".parse().unwrap();
        assert!(u.is_unknown());
        assert_eq!(u.union(&a), a);
        for bad in ["", "cn", "CHN", "CN|", "CN|x"] {
            assert!(bad.parse::<Jurisdiction>().is_err(), "{bad}");
        }
    }

    #[test]
    fn training_risk_is_conservative() {
        let mut tags = RouteTags::default();
        assert_eq!(tags.training_risk(), TrainingRisk::Unknown);
        tags.privacy.insert(PrivacyTag::Eu);
        assert_eq!(tags.training_risk(), TrainingRisk::NoTrain);
        tags.privacy.insert(PrivacyTag::Unknown);
        assert_eq!(tags.training_risk(), TrainingRisk::Unknown);
        tags.privacy.insert(PrivacyTag::CnMayTrain);
        assert_eq!(tags.training_risk(), TrainingRisk::MayTrain);
    }
}
