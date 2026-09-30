//! Identyfikatory i referencje używane w manifeście (reprezentacja tekstowa w TOML/JSON).

use std::fmt;
use std::str::FromStr;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::validate::{is_kebab_case, ManifestError};

/// Identyfikator modułu w kebab-case (np. `voice-stt`).
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, JsonSchema)]
#[serde(transparent)]
pub struct ModuleId(String);

impl ModuleId {
    /// Tworzy identyfikator, sprawdzając kebab-case.
    pub fn new(id: impl Into<String>) -> Result<Self, ManifestError> {
        let id = id.into();
        if is_kebab_case(&id) {
            Ok(Self(id))
        } else {
            Err(ManifestError::InvalidId(id))
        }
    }

    /// Widok tekstowy.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl<'de> Deserialize<'de> for ModuleId {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(deserializer)?;
        Self::new(raw).map_err(serde::de::Error::custom)
    }
}

impl fmt::Display for ModuleId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Referencja do kontraktu: `"<nazwa>-contract@<major>"`, np. `core-bus-contract@1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ContractRef {
    /// Nazwa crate'a kontraktu (kebab-case, kończy się na `-contract`).
    pub name: String,
    /// Wersja główna kontraktu.
    pub major: u32,
}

impl FromStr for ContractRef {
    type Err = ManifestError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let invalid = || ManifestError::InvalidContractRef(s.to_owned());
        let (name, major) = s.split_once('@').ok_or_else(invalid)?;
        if !is_kebab_case(name) || !name.ends_with("-contract") {
            return Err(invalid());
        }
        let major = major.parse::<u32>().map_err(|_| invalid())?;
        Ok(Self {
            name: name.to_owned(),
            major,
        })
    }
}

impl fmt::Display for ContractRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}@{}", self.name, self.major)
    }
}

/// Żądana zdolność (token Brokera, PLAN §8.1): `"<nazwa>"` albo `"<nazwa>(<zakres>)"`,
/// np. `fs.read(%LOCALAPPDATA%/Alfa/models/**)`, `gpu.compute`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Capability {
    /// Nazwa zdolności (segmenty rozdzielone kropką).
    pub name: String,
    /// Zakres (opcjonalny, w nawiasach).
    pub scope: Option<String>,
}

impl FromStr for Capability {
    type Err = ManifestError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let invalid = || ManifestError::InvalidCapability(s.to_owned());
        let (name, scope) = match s.split_once('(') {
            Some((name, rest)) => {
                let scope = rest.strip_suffix(')').ok_or_else(invalid)?;
                if scope.is_empty() {
                    return Err(invalid());
                }
                (name, Some(scope.to_owned()))
            }
            None => (s, None),
        };
        let name_ok = !name.is_empty()
            && name.split('.').all(|seg| {
                !seg.is_empty()
                    && seg
                        .chars()
                        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
            });
        if !name_ok {
            return Err(invalid());
        }
        Ok(Self {
            name: name.to_owned(),
            scope,
        })
    }
}

impl fmt::Display for Capability {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.scope {
            Some(scope) => write!(f, "{}({scope})", self.name),
            None => f.write_str(&self.name),
        }
    }
}

macro_rules! string_repr {
    ($ty:ident, $desc:literal) => {
        impl Serialize for $ty {
            fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
                s.serialize_str(&self.to_string())
            }
        }

        impl<'de> Deserialize<'de> for $ty {
            fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
                String::deserialize(d)?.parse().map_err(serde::de::Error::custom)
            }
        }

        impl JsonSchema for $ty {
            fn schema_name() -> std::borrow::Cow<'static, str> {
                stringify!($ty).into()
            }

            fn json_schema(_gen: &mut schemars::SchemaGenerator) -> schemars::Schema {
                schemars::json_schema!({ "type": "string", "description": $desc })
            }
        }
    };
}

string_repr!(
    ContractRef,
    "Referencja kontraktu: `<nazwa>-contract@<major>`."
);
string_repr!(
    Capability,
    "Żądana zdolność: `<nazwa>` lub `<nazwa>(<zakres>)`."
);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn contract_ref_parses_and_prints() {
        let r: ContractRef = "core-bus-contract@1".parse().unwrap();
        assert_eq!(
            r,
            ContractRef {
                name: "core-bus-contract".into(),
                major: 1
            }
        );
        assert_eq!(r.to_string(), "core-bus-contract@1");
        for bad in [
            "core-bus@1",
            "core-bus-contract",
            "Core-Bus-contract@1",
            "x-contract@a",
        ] {
            assert!(bad.parse::<ContractRef>().is_err(), "{bad}");
        }
    }

    #[test]
    fn capability_parses_scope() {
        let c: Capability = "fs.read(%LOCALAPPDATA%/Alfa/**)".parse().unwrap();
        assert_eq!(c.scope.as_deref(), Some("%LOCALAPPDATA%/Alfa/**"));
        assert_eq!(c.to_string(), "fs.read(%LOCALAPPDATA%/Alfa/**)");
        assert!("gpu.compute".parse::<Capability>().unwrap().scope.is_none());
        for bad in ["", "fs.read(", "fs.read()", "Fs.Read", "a..b"] {
            assert!(bad.parse::<Capability>().is_err(), "{bad}");
        }
    }

    #[test]
    fn module_id_requires_kebab_case() {
        assert!(ModuleId::new("voice-stt").is_ok());
        for bad in ["VoiceStt", "voice_stt", "-x", "x-", "x--y", ""] {
            assert!(ModuleId::new(bad).is_err(), "{bad}");
        }
    }
}
