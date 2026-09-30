//! Typy zdarzenia magistrali wg docs/PLAN.md §13.

use std::fmt;
use std::str::FromStr;

use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

macro_rules! string_id {
    ($(#[$doc:meta])* $name:ident) => {
        $(#[$doc])*
        #[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, JsonSchema)]
        #[serde(transparent)]
        pub struct $name(pub String);

        impl $name {
            /// Tworzy identyfikator z dowolnego tekstu.
            pub fn new(value: impl Into<String>) -> Self {
                Self(value.into())
            }

            /// Widok tekstowy identyfikatora.
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl From<&str> for $name {
            fn from(value: &str) -> Self {
                Self(value.to_owned())
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&self.0)
            }
        }
    };
}

string_id!(
    /// Identyfikator sesji (okna rozmowy / zadania), §11.
    SessionId
);
string_id!(
    /// Identyfikator agentki (persony), §9.
    AgentId
);
string_id!(
    /// Identyfikator przebiegu (jedno wykonanie planu w sesji).
    RunId
);
string_id!(
    /// Identyfikator spanu (krok wewnątrz przebiegu).
    SpanId
);

/// Poziom zdarzenia (PLAN §13: TRACE…AUDIT). Kolejność rosnąca ważności.
#[derive(
    Debug,
    Clone,
    Copy,
    Default,
    PartialEq,
    Eq,
    Hash,
    PartialOrd,
    Ord,
    Serialize,
    Deserialize,
    JsonSchema,
)]
#[serde(rename_all = "lowercase")]
pub enum Level {
    /// Ślad diagnostyczny (bardzo szczegółowy).
    #[default]
    Trace,
    /// Informacje deweloperskie.
    Debug,
    /// Zwykły przebieg pracy.
    Info,
    /// Sytuacja nietypowa, ale obsłużona.
    Warn,
    /// Błąd operacji.
    Error,
    /// Zdarzenie audytowe — zapisuje wyłącznie Broker (PLAN §8.1).
    Audit,
}

/// Rodzaj zdarzenia = strumień z PLAN §13; `Custom` dla zdarzeń modułów
/// (konwencja: `"<moduł>.<obiekt>.<czynność>"`, np. `"voice.dialog.state_changed"`).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum EventKind {
    /// Akcje wrażliwe, decyzje uprawnień, zmiany konfiguracji (strumień Audyt).
    Audit,
    /// Wywołania modeli: dostawca, model, tokeny, koszt, opóźnienie.
    ModelCall,
    /// Narzędzia (fs, shell, sieć…).
    Tool,
    /// Sterowanie GUI (krok + zrzut + migawka UIA).
    Gui,
    /// Potok głosowy.
    Voice,
    /// Błędy, moduły, watchdog.
    Diagnostics,
    /// Zdarzenia interfejsu użytkownika.
    Ui,
    /// Zdarzenie własne modułu (nazwa z przestrzenią modułu).
    Custom(String),
}

impl EventKind {
    const BUILTIN: [(&'static str, EventKind); 7] = [
        ("audit", EventKind::Audit),
        ("model_call", EventKind::ModelCall),
        ("tool", EventKind::Tool),
        ("gui", EventKind::Gui),
        ("voice", EventKind::Voice),
        ("diagnostics", EventKind::Diagnostics),
        ("ui", EventKind::Ui),
    ];

    /// Nazwa tekstowa rodzaju (identyczna z reprezentacją JSON).
    pub fn as_str(&self) -> &str {
        match self {
            EventKind::Custom(name) => name,
            builtin => Self::BUILTIN
                .iter()
                .find(|(_, kind)| kind == builtin)
                .map_or("", |(name, _)| name),
        }
    }
}

impl fmt::Display for EventKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for EventKind {
    type Err = std::convert::Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self::BUILTIN
            .iter()
            .find(|(name, _)| *name == s)
            .map_or_else(|| EventKind::Custom(s.to_owned()), |(_, kind)| kind.clone()))
    }
}

impl Serialize for EventKind {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for EventKind {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(deserializer)?;
        if raw.is_empty() {
            return Err(serde::de::Error::custom("pusty rodzaj zdarzenia"));
        }
        Ok(raw.parse().unwrap_or(EventKind::Custom(raw)))
    }
}

impl JsonSchema for EventKind {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "EventKind".into()
    }

    fn json_schema(_gen: &mut schemars::SchemaGenerator) -> schemars::Schema {
        schemars::json_schema!({
            "type": "string",
            "minLength": 1,
            "description": "Rodzaj zdarzenia: audit | model_call | tool | gui | voice | diagnostics | ui albo nazwa własna modułu (`<moduł>.<obiekt>.<czynność>`)."
        })
    }
}

/// Koszt związany ze zdarzeniem (wywołanie modelu, narzędzie płatne).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Cost {
    /// Tokeny wejściowe.
    pub input_tokens: u64,
    /// Tokeny wyjściowe.
    pub output_tokens: u64,
    /// Koszt w mikro-dolarach (1 USD = 1_000_000), bez przeliczenia na PLN.
    pub micro_usd: u64,
    /// Opóźnienie wywołania w milisekundach, jeśli znane.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latency_ms: Option<u64>,
}

/// Zdarzenie magistrali (PLAN §13). Raz opublikowane jest niezmienne.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[schemars(
    title = "Event",
    description = "Zdarzenie magistrali Alfy (docs/PLAN.md §13), schemat v1."
)]
pub struct Event {
    /// Unikatowy identyfikator zdarzenia.
    pub id: Uuid,
    /// Znacznik czasu UTC nadania zdarzenia.
    pub ts: DateTime<Utc>,
    /// Sesja, w której powstało zdarzenie.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session: Option<SessionId>,
    /// Agentka, która wywołała zdarzenie.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent: Option<AgentId>,
    /// Przebieg planu.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run: Option<RunId>,
    /// Span (krok) wewnątrz przebiegu.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub span: Option<SpanId>,
    /// Rodzaj zdarzenia (strumień).
    pub kind: EventKind,
    /// Poziom.
    pub level: Level,
    /// Ładunek (mały, inline). Duże dane idą jako referencja do `core-log`.
    pub payload: serde_json::Value,
    /// Koszt, jeśli dotyczy.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cost: Option<Cost>,
    /// Hash poprzedniego rekordu w łańcuchu (wypełnia writer logu / Broker, nie wydawca).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prev_hash: Option<String>,
}

impl Event {
    /// Nowe zdarzenie z losowym `id` i bieżącym czasem; pozostałe pola puste.
    pub fn new(kind: EventKind, level: Level, payload: serde_json::Value) -> Self {
        Self {
            id: Uuid::new_v4(),
            ts: Utc::now(),
            session: None,
            agent: None,
            run: None,
            span: None,
            kind,
            level,
            payload,
            cost: None,
            prev_hash: None,
        }
    }

    /// Ustawia sesję (builder).
    #[must_use]
    pub fn with_session(mut self, session: SessionId) -> Self {
        self.session = Some(session);
        self
    }

    /// Ustawia agentkę (builder).
    #[must_use]
    pub fn with_agent(mut self, agent: AgentId) -> Self {
        self.agent = Some(agent);
        self
    }

    /// Ustawia przebieg (builder).
    #[must_use]
    pub fn with_run(mut self, run: RunId) -> Self {
        self.run = Some(run);
        self
    }

    /// Ustawia span (builder).
    #[must_use]
    pub fn with_span(mut self, span: SpanId) -> Self {
        self.span = Some(span);
        self
    }

    /// Ustawia koszt (builder).
    #[must_use]
    pub fn with_cost(mut self, cost: Cost) -> Self {
        self.cost = Some(cost);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kind_round_trip_builtin_and_custom() {
        for (name, kind) in EventKind::BUILTIN {
            assert_eq!(name.parse::<EventKind>().unwrap(), kind);
            assert_eq!(kind.as_str(), name);
        }
        let custom: EventKind = "voice.dialog.state_changed".parse().unwrap();
        assert_eq!(
            custom,
            EventKind::Custom("voice.dialog.state_changed".into())
        );
        assert_eq!(custom.to_string(), "voice.dialog.state_changed");
    }

    #[test]
    fn kind_json_is_plain_string() {
        assert_eq!(
            serde_json::to_string(&EventKind::ModelCall).unwrap(),
            "\"model_call\""
        );
        let parsed: EventKind = serde_json::from_str("\"x.y\"").unwrap();
        assert_eq!(parsed, EventKind::Custom("x.y".into()));
        assert!(serde_json::from_str::<EventKind>("\"\"").is_err());
    }

    #[test]
    fn levels_are_ordered() {
        assert!(Level::Trace < Level::Debug);
        assert!(Level::Error < Level::Audit);
    }
}
