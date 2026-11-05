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

/// Wpis dziennika werdyktów (panel „Zgody”, oś czasu rozmowy, dziennik rodzica).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JudgeLogEntry {
    /// Sesja proszącej agentki.
    pub session: String,
    /// Agentka (gdy znana).
    pub agent: Option<String>,
    /// Narzędzie (np. `shell.run`).
    pub tool: String,
    /// Opis akcji po polsku (z faktów Sędziego Cieni).
    pub summary: String,
    /// Werdykt modelu.
    pub verdict: crate::JudgeVerdict,
    /// Model sędziego.
    pub model: String,
    /// Czas oceny (ms).
    pub took_ms: u64,
    /// Sędzia Cieni przyjął werdykt (`false` — termin minął albo warunki się zmieniły; decyduje właściciel).
    pub accepted: bool,
}

/// Odbiorca dziennika werdyktów (rdzeń: oś czasu rozmowy). Błąd zapisu nie zmienia decyzji.
pub trait JudgeLog: Send + Sync {
    /// Zapisuje jeden werdykt.
    fn record(&self, entry: JudgeLogEntry);
}
