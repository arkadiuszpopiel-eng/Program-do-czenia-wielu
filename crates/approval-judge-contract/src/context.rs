//! Skąd kierowca sędziego bierze kontekst rozmowy (cel, zwięzły zapis, granice, zadanie, pochodzenie modelu).

use crate::JudgeTurn;

/// Kontekst rozmowy proszącej agentki.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct JudgeContext {
    /// Cel zadania (pierwsza wiadomość właściciela albo opis zadania).
    pub goal: String,
    /// Zwięzły zapis, od najstarszych (bez ukrytego rozumowania).
    pub transcript: Vec<JudgeTurn>,
    /// Granice podane przez właściciela („nie wypychaj”, „nie usuwaj”).
    pub limits: Vec<String>,
    /// Bieżące zadanie (przebieg główny) — dla licznika bezpiecznika „w zadaniu”.
    pub task: Option<String>,
    /// Przebieg na modelu lokalnym albo pochodzenie nieznane: sędzia wyłącznie lokalny (zapis rozmowy nie
    /// wychodzi do chmury, której właściciel nie używa w tej rozmowie).
    pub local_only: bool,
}

/// Źródło kontekstu (rdzeń: historia sesji). `None` — rozmowy nie znaleziono (decyduje właściciel).
pub trait JudgeContextSource: Send + Sync {
    /// Kontekst dla sesji i agentki z prośby.
    fn context(&self, session: &str, agent: Option<&str>) -> Option<JudgeContext>;
}
