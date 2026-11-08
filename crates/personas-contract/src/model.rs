//! Model danych: persona (stała tożsamość), rola (zmienna), szablon obsady. Serializowalne do `.alfa`.

use std::collections::{BTreeMap, BTreeSet};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::ids::{ColorToken, PersonaId, RoleId, TemplateId, is_valid_id};
use crate::text::fold;

/// Odmiana imienia przez przypadki (do rozpoznawania adresatki i poleceń: „Delto”, „Delcie”…).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct NameForms {
    /// Mianownik („Delta”).
    pub nominative: String,
    /// Dopełniacz („Delty”).
    pub genitive: String,
    /// Celownik („Delcie”).
    pub dative: String,
    /// Biernik („Deltę”).
    pub accusative: String,
    /// Narzędnik („Deltą”).
    pub instrumental: String,
    /// Miejscownik („Delcie”).
    pub locative: String,
    /// Wołacz („Delto”).
    pub vocative: String,
}

/// Przypadek gramatyczny formy imienia.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Case {
    /// Mianownik.
    Nominative,
    /// Dopełniacz.
    Genitive,
    /// Celownik.
    Dative,
    /// Biernik.
    Accusative,
    /// Narzędnik.
    Instrumental,
    /// Miejscownik.
    Locative,
    /// Wołacz.
    Vocative,
}

impl NameForms {
    /// Wszystkie przypadki jednakowe (np. imię nieodmienne z Kreatora).
    pub fn uniform(name: &str) -> Self {
        let n = name.to_owned();
        Self {
            nominative: n.clone(),
            genitive: n.clone(),
            dative: n.clone(),
            accusative: n.clone(),
            instrumental: n.clone(),
            locative: n.clone(),
            vocative: n,
        }
    }

    /// Pary (przypadek, forma).
    pub fn all(&self) -> [(Case, &str); 7] {
        [
            (Case::Nominative, self.nominative.as_str()),
            (Case::Genitive, self.genitive.as_str()),
            (Case::Dative, self.dative.as_str()),
            (Case::Accusative, self.accusative.as_str()),
            (Case::Instrumental, self.instrumental.as_str()),
            (Case::Locative, self.locative.as_str()),
            (Case::Vocative, self.vocative.as_str()),
        ]
    }

    /// Przypadki, którym odpowiada słowo (po `fold`).
    pub fn cases_of(&self, folded_word: &str) -> BTreeSet<Case> {
        self.all()
            .into_iter()
            .filter(|(_, form)| fold(form) == folded_word)
            .map(|(case, _)| case)
            .collect()
    }
}

/// Biblia głosu (docs/PERSONAS.md §2, PLAN §6.6) — dane konfiguracyjne, eksportowane w `.alfa`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct VoiceBible {
    /// Język i styl („polski, czysta polszczyzna; swobodnie wtrąca terminy EN”).
    pub language: String,
    /// Postrzegany wiek (młoda dorosła: 18–25).
    pub perceived_age: u8,
    /// Barwa.
    pub timbre: String,
    /// Rejestr.
    pub register: String,
    /// Tempo.
    pub tempo: String,
    /// Energia.
    pub energy: String,
    /// Zakres emocji.
    pub emotion_range: String,
    /// Prompt do voice design (bez słów „girl / cute / child”).
    pub design_prompt: String,
    /// Pochodzenie i zgoda (brak klonów prawdziwych osób).
    pub provenance: String,
}

/// Persona: stała tożsamość agentki (zawsze żeńska). Głos idzie za personą.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Persona {
    /// Identyfikator (`alfa`…, własne: `[a-z][a-z0-9-]{1,31}`).
    pub id: PersonaId,
    /// Imię.
    pub name: String,
    /// Glif (α β γ δ) — zawsze obok koloru i imienia.
    pub glyph: char,
    /// Token koloru akcentu z `packages/ui-kit` (nazwa, nie wartość).
    pub color: ColorToken,
    /// Charakter („ciepła, spokojna, konkretna”).
    pub character: String,
    /// Frazy wywoławcze („Hej Alfa”).
    pub wake_phrases: Vec<String>,
    /// Odmiana imienia.
    pub forms: NameForms,
    /// Biblia głosu.
    pub voice: VoiceBible,
    /// Wbudowana (nieusuwalna) czy z Kreatora.
    #[serde(default)]
    pub builtin: bool,
}

/// Rola: zestaw zadań, narzędzi i uprawnień w sesji. Uprawnienia idą za rolą (od F3 tokeny Brokera).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Role {
    /// Identyfikator.
    pub id: RoleId,
    /// Nazwa (PL, forma żeńska: „Krytyczka / Weryfikatorka”).
    pub name: String,
    /// Zadania roli (opis dla UI).
    pub description: String,
    /// Fragment promptu systemowego roli (rodzaj żeński).
    pub prompt: String,
    /// Klasa zadań dla routera (`conversation`, `planning`, `code`, `gui-vision`, `summarize`…).
    pub model_policy: String,
    /// Narzędzia roli (pod sufitem sesji; F3 → tokeny zdolności).
    #[serde(default)]
    pub tools: Vec<String>,
    /// Tylko odczyt (Krytyczka).
    #[serde(default)]
    pub read_only: bool,
    /// Praca na niezaufanych źródłach w izolacji (Badaczka).
    #[serde(default)]
    pub untrusted_isolated: bool,
    /// Rola tworząca wyniki, które weryfikuje Krytyczka (Wykonawczyni, Koderka, Pisarka).
    #[serde(default)]
    pub author: bool,
    /// Najwyżej jedna agentka w obsadzie (Dyrygentka, Mówczyni).
    #[serde(default)]
    pub unique: bool,
    /// Wbudowana czy własna.
    #[serde(default)]
    pub builtin: bool,
}

/// Szablon obsady (Standard, Solo, Kodowanie, Badania, własne).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct CastTemplate {
    /// Identyfikator.
    pub id: TemplateId,
    /// Nazwa (PL).
    pub name: String,
    /// Przydział ról. Dla „Solo” pusty — obsadę buduje `Cast::solo`.
    pub assignments: BTreeMap<PersonaId, BTreeSet<RoleId>>,
    /// Wbudowany czy własny.
    #[serde(default)]
    pub builtin: bool,
}

/// Błędy danych persony / roli / szablonu (Kreator).
#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "error", rename_all = "snake_case")]
pub enum ModelError {
    /// Niepoprawny identyfikator.
    #[error("niepoprawny identyfikator `{0}` (dozwolone: [a-z][a-z0-9-]{{1,31}})")]
    InvalidId(String),
    /// Wymagane pole jest puste albo za długie.
    #[error("pole `{0}` jest puste lub za długie")]
    InvalidField(String),
    /// Postrzegany wiek poza zakresem 18–25.
    #[error("postrzegany wiek {0} poza zakresem 18–25")]
    AgeOutOfRange(u8),
    /// Zakazane słowo w prompcie voice design.
    #[error("prompt głosu zawiera zakazane słowo `{0}`")]
    ForbiddenVoiceWord(String),
    /// Fraza wywoławcza nie zawiera imienia.
    #[error("fraza wywoławcza `{0}` nie zawiera imienia")]
    WakePhraseWithoutName(String),
    /// Identyfikator, imię, forma imienia albo glif koliduje z istniejącą personą/rolą/szablonem.
    #[error("kolizja z istniejącym elementem: {0}")]
    Conflict(String),
    /// Elementu wbudowanego nie można nadpisać.
    #[error("element wbudowany `{0}` nie może być nadpisany")]
    Builtin(String),
}

/// Słowa zakazane w promptach voice design (docs/PERSONAS.md §2).
pub const FORBIDDEN_VOICE_WORDS: [&str; 6] =
    ["girl", "cute", "child", "kid", "dziewczynka", "dziecko"];

fn field(name: &str, value: &str, max: usize) -> Result<(), ModelError> {
    let len = value.chars().count();
    if value.trim().is_empty() || len > max {
        return Err(ModelError::InvalidField(name.to_owned()));
    }
    Ok(())
}

impl Persona {
    /// Walidacja danych persony (Kreator; wbudowane przechodzą ją w testach).
    pub fn validate(&self) -> Result<(), ModelError> {
        if !is_valid_id(self.id.as_str()) {
            return Err(ModelError::InvalidId(self.id.to_string()));
        }
        field("name", &self.name, 32)?;
        field("character", &self.character, 200)?;
        field("color", self.color.as_str(), 64)?;
        if self.glyph.is_whitespace() || self.glyph.is_control() {
            return Err(ModelError::InvalidField("glyph".into()));
        }
        if self.forms.nominative != self.name {
            return Err(ModelError::InvalidField("forms.nominative".into()));
        }
        for (_, form) in self.forms.all() {
            field("forms", form, 32)?;
        }
        if self.wake_phrases.is_empty() {
            return Err(ModelError::InvalidField("wake_phrases".into()));
        }
        let name = fold(&self.name);
        for phrase in &self.wake_phrases {
            if !fold(phrase).split_whitespace().any(|w| w == name) {
                return Err(ModelError::WakePhraseWithoutName(phrase.clone()));
            }
        }
        self.voice.validate()
    }
}

impl VoiceBible {
    /// Wiek 18–25, wymagane pola, bez zakazanych słów w prompcie głosu.
    pub fn validate(&self) -> Result<(), ModelError> {
        if !(18..=25).contains(&self.perceived_age) {
            return Err(ModelError::AgeOutOfRange(self.perceived_age));
        }
        for (name, value) in [
            ("voice.language", &self.language),
            ("voice.timbre", &self.timbre),
            ("voice.tempo", &self.tempo),
            ("voice.design_prompt", &self.design_prompt),
            ("voice.provenance", &self.provenance),
        ] {
            field(name, value, 400)?;
        }
        let prompt = fold(&self.design_prompt);
        let words: Vec<&str> = prompt.split(|c: char| !c.is_alphanumeric()).collect();
        match FORBIDDEN_VOICE_WORDS.iter().find(|w| words.contains(w)) {
            Some(word) => Err(ModelError::ForbiddenVoiceWord((*word).to_owned())),
            None => Ok(()),
        }
    }
}

impl Role {
    /// Walidacja roli (Kreator).
    pub fn validate(&self) -> Result<(), ModelError> {
        if !is_valid_id(self.id.as_str()) {
            return Err(ModelError::InvalidId(self.id.to_string()));
        }
        field("name", &self.name, 64)?;
        field("prompt", &self.prompt, 4000)?;
        field("model_policy", &self.model_policy, 64)
    }
}

impl CastTemplate {
    /// Walidacja szablonu (identyfikator i nazwa; przydział sprawdza `Catalog::validate_cast`).
    pub fn validate(&self) -> Result<(), ModelError> {
        if !is_valid_id(self.id.as_str()) {
            return Err(ModelError::InvalidId(self.id.to_string()));
        }
        field("name", &self.name, 64)
    }
}
