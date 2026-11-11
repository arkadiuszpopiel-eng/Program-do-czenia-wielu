//! Dobór modelu według trudności (zgłoszenie właściciela 2026-10-14, plan P12, macierz Delty H4):
//! każdy kandydat ma szczebel — lekki (lokalny mały albo tani), średni, mocny — a zadanie zaczyna
//! od szczebla swojej trudności. Wyższe szczeble zostają za nim jako zapas, niższe na końcu.
//! Po odrzuceniu odpowiedzi (Krytyczka) zadanie idzie szczebel wyżej; sprawdzanie — na szczeblu
//! z ustawień (domyślnie lekkim).

use std::collections::BTreeMap;

use providers_contract::{Difficulty, Strategy};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::TaskClass;
use crate::candidate::Candidate;

/// Szczebel modelu. Kolejność: lekki < średni < mocny.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum Rung {
    /// Mały model lokalny albo tani model API (`mini`, `flash`, `haiku`, poniżej8 mld parametrów).
    Light,
    /// Model główny rodziny albo lokalny średni (do 40 mld parametrów, np. Qwen 27B).
    Standard,
    /// Najmocniejszy (`opus`, `pro`, `sol`, ponad 40 mld parametrów).
    Strong,
}

impl Rung {
    /// Szczebel, od którego zaczyna zadanie o tej trudności.
    pub fn for_difficulty(d: Difficulty) -> Self {
        match d {
            Difficulty::Easy => Self::Light,
            Difficulty::Medium => Self::Standard,
            Difficulty::Hard => Self::Strong,
        }
    }

    /// Szczebel wyżej (mocny zostaje mocnym).
    #[must_use]
    pub fn up(self) -> Self {
        match self {
            Self::Light => Self::Standard,
            Self::Standard | Self::Strong => Self::Strong,
        }
    }

    /// Nazwa dla człowieka.
    pub fn label(self) -> &'static str {
        match self {
            Self::Light => "lekki",
            Self::Standard => "średni",
            Self::Strong => "mocny",
        }
    }

    fn index(self) -> u8 {
        match self {
            Self::Light => 0,
            Self::Standard => 1,
            Self::Strong => 2,
        }
    }
}

/// Klasy, w których działa dobór według trudności. Głos-szybka (czas odpowiedzi), embeddingi
/// (jeden model osadzeń) i streszczenia (zawsze lekkie, w tle) mają stałą kolejność.
pub const DIFFICULTY_CLASSES: [TaskClass; 4] = [
    TaskClass::Conversation,
    TaskClass::Code,
    TaskClass::Planning,
    TaskClass::GuiVision,
];

/// Najniższa trudność klasy: kod, planowanie i sterowanie ekranem nie idą na najlżejszy model.
pub fn class_floor(class: TaskClass) -> Difficulty {
    match class {
        TaskClass::Code | TaskClass::Planning | TaskClass::GuiVision => Difficulty::Medium,
        _ => Difficulty::Easy,
    }
}

/// Klasa zadań z „klasy modelu” roli (`model_policy`) albo z nazwy klasy (`code`, `gui_vision`).
pub fn class_from_policy(policy: &str) -> Option<TaskClass> {
    match policy {
        "code" => Some(TaskClass::Code),
        "planning" | "review" | "research" => Some(TaskClass::Planning),
        "gui-vision" | "gui_vision" => Some(TaskClass::GuiVision),
        "summarize" => Some(TaskClass::Summarize),
        "conversation" | "chat" => Some(TaskClass::Conversation),
        "voice_fast" => Some(TaskClass::VoiceFast),
        "embeddings" => Some(TaskClass::Embeddings),
        _ => None,
    }
}

/// Liczba parametrów z nazwy modelu w miliardach (`qwen3-27b-instruct` → 27, `bielik-4.5b` → 4,5).
pub fn params_billions(model: &str) -> Option<f32> {
    model
        .to_ascii_lowercase()
        .split(['-', '_', '/', ':'])
        .find_map(|part| {
            let num = part.strip_suffix('b')?;
            let ok = !num.is_empty() && num.chars().all(|c| c.is_ascii_digit() || c == '.');
            ok.then(|| num.parse::<f32>().ok()).flatten()
        })
}

fn has_word(model: &str, words: &[&str]) -> bool {
    model
        .to_ascii_lowercase()
        .split(['-', '_', '.', '/', ':'])
        .any(|part| words.contains(&part))
}

/// Szczebel z nazwy modelu: najpierw liczba parametrów, potem słowa rodzin (`mini`, `opus`…).
/// Nieznany model lokalny i nieznany model API — średni.
pub fn rung_by_name(model: &str) -> Rung {
    if let Some(b) = params_billions(model) {
        return if b < 8.0 {
            Rung::Light
        } else if b <= 40.0 {
            Rung::Standard
        } else {
            Rung::Strong
        };
    }
    if has_word(
        model,
        &[
            "nano", "lite", "tiny", "mini", "flash", "haiku", "small", "fast", "instant",
        ],
    ) {
        Rung::Light
    } else if has_word(
        model,
        &[
            "opus", "pro", "ultra", "max", "sol", "fable", "mythos", "large", "reasoner",
        ],
    ) {
        Rung::Strong
    } else {
        Rung::Standard
    }
}

/// Ustawienia doboru według trudności (Ustawienia → Dobór modelu).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct DifficultyPolicy {
    /// Dobór włączony; wyłączony — kolejność kolejek bez zmian.
    pub enabled: bool,
    /// Strategia progów trudności.
    pub strategy: Strategy,
    /// Po odrzuceniu odpowiedzi przez Krytyczkę poprawka idzie szczebel wyżej.
    pub escalate: bool,
    /// Trudność sprawdzania odpowiedzi (Krytyczka): łatwe = lekki model sprawdza.
    pub review: Difficulty,
    /// Przy równym szczeblu najpierw model lokalny (bez kosztu), potem API.
    pub local_first: bool,
    /// Szczeble ustawione ręcznie: `dostawca:model` → szczebel (zastępuje rozpoznanie po nazwie).
    pub rungs: BTreeMap<String, Rung>,
    /// Dobór zmienia też kolejność kolejek ustawionych ręcznie (konta, wspólna). Wyłączone —
    /// ręczna kolejka klasy ma pierwszeństwo, dobór działa tylko w klasach bez kolejki
    /// (podniesienie po odrzuceniu odpowiedzi działa zawsze).
    pub reorder_queues: bool,
}

impl Default for DifficultyPolicy {
    fn default() -> Self {
        Self {
            enabled: true,
            strategy: Strategy::Balanced,
            escalate: true,
            review: Difficulty::Easy,
            local_first: true,
            rungs: BTreeMap::new(),
            reorder_queues: false,
        }
    }
}

/// Kandydat z jego szczeblem do ułożenia kolejności.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ranked {
    /// Kandydat.
    pub candidate: Candidate,
    /// Szczebel.
    pub rung: Rung,
    /// Trasa lokalna.
    pub local: bool,
}

/// Kolejność prób od szczebla `start`: najpierw szczeble ≥ `start` rosnąco, potem niższe
/// malejąco (zapas). W obrębie szczebla — lokalne pierwsze (gdy `local_first`), dalej kolejność
/// kolejki. `last` (model odrzucony) idzie na sam koniec.
pub fn order_by_rung(
    list: Vec<Ranked>,
    start: Rung,
    local_first: bool,
    last: Option<&Candidate>,
) -> Vec<Candidate> {
    let mut list = list;
    list.sort_by_key(|r| {
        let demoted = last == Some(&r.candidate);
        let (side, distance) = if r.rung >= start {
            (0, r.rung.index() - start.index())
        } else {
            (1, start.index() - r.rung.index())
        };
        let place = u8::from(local_first && !r.local);
        (demoted, side, distance, place)
    });
    list.into_iter().map(|r| r.candidate).collect()
}

/// Uzasadnienie doboru w decyzji (oś czasu: „trudne — kod, rozumowanie → mocny”).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct DifficultyNote {
    /// Trudność zadania.
    pub difficulty: Difficulty,
    /// Wynik oceny 0–100 (brak — trudność podana przez wywołującego albo sprawdzanie).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub score: Option<u8>,
    /// Powody po ludzku.
    pub reasons: String,
    /// Szczebel, od którego zaczęto.
    pub start: Rung,
    /// Model, którego odpowiedź odrzucono (podniesienie).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub escalated_from: Option<Candidate>,
    /// Klasa ma kolejkę ustawioną ręcznie i zostaje w jej kolejności (bez podniesienia).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub kept_queue: bool,
}

impl DifficultyNote {
    /// Czy dobór zmienia kolejność prób.
    pub fn reorders(&self) -> bool {
        !self.kept_queue || self.escalated_from.is_some()
    }
}

impl DifficultyNote {
    /// Jedno zdanie dla osi czasu.
    pub fn summary(&self) -> String {
        let mut text = format!(
            "{} ({}) → szczebel {}",
            self.difficulty.label(),
            self.reasons,
            self.start.label()
        );
        if let Some(from) = &self.escalated_from {
            text.push_str(&format!(", podniesione po odrzuceniu odpowiedzi {from}"));
        } else if self.kept_queue {
            text = format!(
                "{} ({}) → kolejka ustawiona ręcznie, bez zmian",
                self.difficulty.label(),
                self.reasons
            );
        }
        text
    }
}

#[cfg(test)]
#[path = "rung_tests.rs"]
mod tests;
