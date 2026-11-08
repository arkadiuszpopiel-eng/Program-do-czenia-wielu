//! Dobór modelu według trudności (Ustawienia → Dobór modelu): łatwe zadania na lekkim modelu,
//! kod na średnim, trudne na mocnym; sprawdzanie Krytyczki i podnoszenie po odrzuceniu.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// Trudność zadania.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RoutingLevel {
    Easy,
    Medium,
    Hard,
}

/// Szczebel modelu.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RoutingRung {
    Light,
    Standard,
    Strong,
}

/// Strategia progów trudności.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RoutingStrategy {
    Economy,
    Balanced,
    Quality,
}

/// Ustawienia doboru.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ModelRoutingSettings {
    /// Dobór włączony.
    pub enabled: bool,
    pub strategy: RoutingStrategy,
    /// Poprawka po odrzuceniu odpowiedzi na mocniejszym modelu.
    pub escalate: bool,
    /// Trudność sprawdzania odpowiedzi (Krytyczka).
    pub review: RoutingLevel,
    /// Przy równym szczeblu najpierw model lokalny.
    pub local_first: bool,
    /// Szczeble ustawione ręcznie: `dostawca:model` → szczebel.
    pub rungs: BTreeMap<String, RoutingRung>,
    /// Dobór zmienia też kolejność kolejek ustawionych ręcznie (domyślnie nie — kolejka ręczna
    /// ma pierwszeństwo).
    pub reorder_queues: bool,
}

/// Model w doborze: kandydat Routera z jego szczeblem.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct RoutingModel {
    /// `dostawca:model`.
    pub candidate: String,
    /// Nazwa dostawcy („OpenAI”, „Model lokalny”).
    pub provider_name: String,
    /// Model u dostawcy.
    pub model: String,
    /// Trasa lokalna.
    pub local: bool,
    /// Szczebel obowiązujący.
    pub rung: RoutingRung,
    /// Szczebel rozpoznany automatycznie (po nazwie, lekki dostawca lokalny).
    pub automatic: RoutingRung,
}

/// Ustawienia i modele, które Router bierze pod uwagę.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ModelRouting {
    pub settings: ModelRoutingSettings,
    pub models: Vec<RoutingModel>,
}

/// Podgląd: jak Router potraktuje przykładowe zadanie.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct RoutingPreview {
    /// Trudność (brak — klasa bez doboru albo dobór wyłączony).
    pub difficulty: Option<RoutingLevel>,
    /// Wynik oceny 0–100.
    pub score: Option<u8>,
    /// Powody oceny po ludzku.
    pub reasons: String,
    /// Szczebel startowy.
    pub start: Option<RoutingRung>,
    /// Klasa ma kolejkę ustawioną ręcznie i zostaje w jej kolejności.
    pub kept_queue: bool,
    /// Kolejność prób (pierwszy odpowiada, reszta to zapas).
    pub order: Vec<RoutingModel>,
}
