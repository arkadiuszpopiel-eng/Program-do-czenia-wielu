//! Wskazówka trasy w żądaniu (`RequestMeta::route`): trudność zadania, klasa roli, sprawdzenie
//! Krytyczki i podniesienie do mocniejszego modelu po odrzuceniu odpowiedzi. Wypełnia ją
//! wywołujący (agent-runtime), czyta Router; dostawcy ją pomijają (nie trafia do promptu).

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Trudność zadania. Kolejność ma znaczenie: łatwe < średnie < trudne.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum Difficulty {
    /// Krótkie pytanie, prosta prośba — wystarczy lekki, tani model.
    Easy,
    /// Zwykła praca: kod, wyjaśnienie, kilka kroków — model średni.
    Medium,
    /// Rozumowanie, projekt, długi kod, wiele kroków naraz — najmocniejszy model.
    Hard,
}

impl Difficulty {
    /// Wszystkie poziomy od najłatwiejszego.
    pub const ALL: [Self; 3] = [Self::Easy, Self::Medium, Self::Hard];

    /// Nazwa dla człowieka.
    pub fn label(self) -> &'static str {
        match self {
            Self::Easy => "łatwe",
            Self::Medium => "średnie",
            Self::Hard => "trudne",
        }
    }

    /// Poziom wyżej (trudne zostaje trudnym).
    #[must_use]
    pub fn up(self) -> Self {
        match self {
            Self::Easy => Self::Medium,
            Self::Medium | Self::Hard => Self::Hard,
        }
    }
}

/// Wskazówka trasy. Pusta = Router sam ocenia trudność z treści żądania.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RouteHint {
    /// Klasa zadań roli (`model_policy` roli albo nazwa klasy, np. `code`) — zastępuje klasę
    /// dostawcy Routera (delegacja do Koderki idzie kolejką „Kod”).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub class: Option<String>,
    /// Trudność ustalona przez wywołującego (zastępuje ocenę treści).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub difficulty: Option<Difficulty>,
    /// Treść zadania do oceny trudności zamiast ostatniej wiadomości (cel przebiegu agentki —
    /// kolejne tury z wynikami narzędzi nie zmieniają oceny). Skrócona do [`TASK_EXCERPT`] znaków.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task: Option<String>,
    /// Sprawdzenie cudzej odpowiedzi (Krytyczka) — Router bierze poziom z ustawień sprawdzania.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub review: bool,
    /// Model (`dostawca:model`), którego odpowiedź odrzucono — Router zaczyna od szczebla wyżej.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub escalate_from: Option<String>,
}

/// Najwięcej znaków treści zadania we wskazówce.
pub const TASK_EXCERPT: usize = 2_000;

impl RouteHint {
    /// Czy wskazówka jest pusta (nie zapisywana w żądaniu).
    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }

    /// Ustawia treść zadania do oceny (skróconą do [`TASK_EXCERPT`] znaków).
    pub fn set_task(&mut self, text: &str) {
        self.task = Some(text.chars().take(TASK_EXCERPT).collect());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn difficulty_orders_and_goes_up_to_hard() {
        assert!(Difficulty::Easy < Difficulty::Medium && Difficulty::Medium < Difficulty::Hard);
        assert_eq!(Difficulty::Easy.up(), Difficulty::Medium);
        assert_eq!(Difficulty::Hard.up(), Difficulty::Hard);
        assert_eq!(Difficulty::Medium.label(), "średnie");
    }

    #[test]
    fn empty_hint_is_not_serialized_fields() {
        let hint = RouteHint::default();
        assert!(hint.is_empty());
        assert_eq!(serde_json::to_value(&hint).unwrap(), serde_json::json!({}));
        let full = RouteHint {
            class: Some("code".into()),
            difficulty: Some(Difficulty::Hard),
            task: None,
            review: true,
            escalate_from: Some("local:qwen".into()),
        };
        let back: RouteHint = serde_json::from_value(serde_json::to_value(&full).unwrap()).unwrap();
        assert_eq!(back, full);
        let mut long = RouteHint::default();
        long.set_task(&"ą".repeat(5_000));
        assert_eq!(long.task.unwrap().chars().count(), TASK_EXCERPT);
    }
}
