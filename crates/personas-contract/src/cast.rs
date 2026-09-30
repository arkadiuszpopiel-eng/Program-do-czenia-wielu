//! Obsada ról w sesji: persona → zbiór ról. Walidacja i operacje (deterministyczne, bez stanu).

use std::collections::{BTreeMap, BTreeSet};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::ids::{PersonaId, RoleId, TemplateId};

/// Obsada sesji. Dyrygentka i Mówczyni wynikają z przydziału (jedno źródło prawdy).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Cast {
    /// Szablon, z którego powstała obsada (`None` po ręcznych zmianach też jest dozwolone).
    #[serde(default)]
    pub template: Option<TemplateId>,
    /// Sesja głosowa — wymaga dokładnie jednej Mówczyni.
    #[serde(default)]
    pub voice: bool,
    /// Przydział ról (persony bez ról nie występują).
    pub assignments: BTreeMap<PersonaId, BTreeSet<RoleId>>,
}

/// Kto weryfikuje wynik autorki (Krytyczka ≠ autorka, gdy to możliwe).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", content = "persona", rename_all = "snake_case")]
pub enum Verifier {
    /// Agentka w roli Krytyczki (inna niż autorka).
    Critic(PersonaId),
    /// Jedyna Krytyczka jest autorką — weryfikację przejmuje inna agentka z obsady
    /// (kolejno: Myślicielka, Dyrygentka, dowolna).
    Substitute(PersonaId),
    /// Autorka jest jedyną agentką w obsadzie (np. Solo) — samoweryfikacja, oznaczana w UI.
    SelfCheck(PersonaId),
}

/// Ostrzeżenia walidacji (obsada poprawna, ale warto ją poprawić).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "warning", content = "persona", rename_all = "snake_case")]
pub enum CastWarning {
    /// Brak Krytyczki — wyniki nie będą weryfikowane przed „gotowe”.
    NoCritic,
    /// Krytyczka ma też rolę autorki; jej własne wyniki zweryfikuje zastępczyni.
    CriticAlsoAuthor(PersonaId),
}

/// Różnica dwóch obsad (do zdarzeń `personas.role.assigned` i dziennika).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct CastDiff {
    /// Nowe przydziały (persona, rola).
    pub assigned: Vec<(PersonaId, RoleId)>,
    /// Odebrane przydziały (persona, rola).
    pub removed: Vec<(PersonaId, RoleId)>,
}

/// Błędy walidacji obsady.
#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "error", rename_all = "snake_case")]
pub enum CastError {
    /// Obsada bez żadnej agentki.
    #[error("obsada jest pusta")]
    Empty,
    /// Nieznana persona.
    #[error("nieznana persona `{0}`")]
    UnknownPersona(PersonaId),
    /// Nieznana rola.
    #[error("nieznana rola `{0}`")]
    UnknownRole(RoleId),
    /// Nieznany szablon obsady.
    #[error("nieznany szablon obsady `{0}`")]
    UnknownTemplate(TemplateId),
    /// Brak Dyrygentki (musi być dokładnie jedna).
    #[error("obsada musi mieć dokładnie jedną Dyrygentkę")]
    NoConductor,
    /// Rola unikalna przydzielona kilku agentkom.
    #[error("rolę `{role}` może mieć tylko jedna agentka, a ma ją {}", holders.len())]
    MultipleHolders {
        /// Rola.
        role: RoleId,
        /// Agentki z tą rolą.
        holders: Vec<PersonaId>,
    },
    /// Sesja głosowa bez Mówczyni.
    #[error("sesja głosowa wymaga dokładnie jednej Mówczyni")]
    NoSpeakerInVoiceSession,
}

impl Cast {
    /// Obsada z przydziału (usuwa persony bez ról).
    pub fn new(
        template: Option<TemplateId>,
        voice: bool,
        assignments: BTreeMap<PersonaId, BTreeSet<RoleId>>,
    ) -> Self {
        let mut cast = Self {
            template,
            voice,
            assignments,
        };
        cast.normalize();
        cast
    }

    /// Obsada „Solo”: jedna agentka ze wszystkimi podanymi rolami.
    pub fn solo(persona: PersonaId, roles: impl IntoIterator<Item = RoleId>, voice: bool) -> Self {
        let set: BTreeSet<RoleId> = roles.into_iter().collect();
        Self::new(
            Some(TemplateId::solo()),
            voice,
            BTreeMap::from([(persona, set)]),
        )
    }

    /// Agentki z daną rolą (w kolejności identyfikatorów).
    pub fn holders(&self, role: &RoleId) -> Vec<PersonaId> {
        self.assignments
            .iter()
            .filter(|(_, roles)| roles.contains(role))
            .map(|(p, _)| p.clone())
            .collect()
    }

    /// Dyrygentka (po walidacji zawsze jest).
    pub fn conductor(&self) -> Option<PersonaId> {
        self.holders(&RoleId::conductor()).into_iter().next()
    }

    /// Mówczyni, jeśli jest.
    pub fn speaker(&self) -> Option<PersonaId> {
        self.holders(&RoleId::speaker()).into_iter().next()
    }

    /// Role agentki (pusty zbiór, gdy nie gra w tej obsadzie).
    pub fn roles_of(&self, persona: &PersonaId) -> BTreeSet<RoleId> {
        self.assignments.get(persona).cloned().unwrap_or_default()
    }

    /// Agentki z co najmniej jedną rolą.
    pub fn personas(&self) -> Vec<PersonaId> {
        self.assignments.keys().cloned().collect()
    }

    /// Przydziela rolę; `exclusive` odbiera ją pozostałym („przejmij”).
    pub fn assign(&mut self, persona: &PersonaId, role: &RoleId, exclusive: bool) {
        if exclusive {
            for roles in self.assignments.values_mut() {
                roles.remove(role);
            }
        }
        self.assignments
            .entry(persona.clone())
            .or_default()
            .insert(role.clone());
        self.normalize();
    }

    /// W sesji głosowej bez Mówczyni rolę dostaje Dyrygentka.
    pub fn ensure_speaker(&mut self) {
        if self.voice
            && self.speaker().is_none()
            && let Some(conductor) = self.conductor()
        {
            self.assign(&conductor, &RoleId::speaker(), true);
        }
    }

    /// Odbiera rolę agentce.
    pub fn unassign(&mut self, persona: &PersonaId, role: &RoleId) {
        if let Some(roles) = self.assignments.get_mut(persona) {
            roles.remove(role);
        }
        self.normalize();
    }

    /// Kto weryfikuje wynik `author` (Krytyczka ≠ autorka, gdy to możliwe); `None`, gdy
    /// w obsadzie nie ma Krytyczki.
    pub fn verifier_for(&self, author: &PersonaId) -> Option<Verifier> {
        let critics = self.holders(&RoleId::critic());
        if critics.is_empty() {
            return None;
        }
        if let Some(critic) = critics.into_iter().find(|p| p != author) {
            return Some(Verifier::Critic(critic));
        }
        let substitute = [RoleId::thinker(), RoleId::conductor()]
            .iter()
            .flat_map(|role| self.holders(role))
            .chain(self.personas())
            .find(|p| p != author);
        Some(substitute.map_or_else(|| Verifier::SelfCheck(author.clone()), Verifier::Substitute))
    }

    /// Różnica względem wcześniejszej obsady.
    pub fn diff_from(&self, before: &Cast) -> CastDiff {
        let pairs = |cast: &Cast| -> BTreeSet<(PersonaId, RoleId)> {
            cast.assignments
                .iter()
                .flat_map(|(p, roles)| roles.iter().map(move |r| (p.clone(), r.clone())))
                .collect()
        };
        let (old, new) = (pairs(before), pairs(self));
        CastDiff {
            assigned: new.difference(&old).cloned().collect(),
            removed: old.difference(&new).cloned().collect(),
        }
    }

    fn normalize(&mut self) {
        self.assignments.retain(|_, roles| !roles.is_empty());
    }
}
