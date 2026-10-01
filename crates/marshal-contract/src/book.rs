//! Księga reguł: propozycje (szkice → walidacja → konflikty), zatwierdzanie wyłącznie przez
//! użytkownika, cofanie, polityka efektywna. Stan serializowalny (trwałość).

use std::collections::BTreeMap;

use personas_contract::PersonaId;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::check::{Ceiling, Conflict, Violation, check_rule, conflicts};
use crate::policy::{EffectivePolicy, compose};
use crate::rule::{Rule, RuleId, parse_rule};

/// Kto zatwierdza / cofa.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "via", content = "id", rename_all = "snake_case")]
pub enum Approver {
    /// Użytkownik w UI (fizyczne kliknięcie).
    UserInterface,
    /// Użytkownik głosem.
    UserVoice,
    /// Agentka — nigdy nie zatwierdza reguł.
    Agent(PersonaId),
}

impl Approver {
    /// Czy to użytkownik.
    pub fn is_user(&self) -> bool {
        matches!(self, Self::UserInterface | Self::UserVoice)
    }
}

/// Stan propozycji.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ProposalStatus {
    /// Czeka na decyzję użytkownika.
    Pending,
    /// Zatwierdzona — reguły aktywne.
    Approved,
    /// Odrzucona.
    Rejected,
}

/// Odrzucony szkic (z powodami).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct RejectedDraft {
    /// Szkic (niezaufany).
    pub draft: serde_json::Value,
    /// Powody.
    pub errors: Vec<String>,
}

/// Propozycja Marszałka.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Proposal {
    /// Numer.
    pub id: u64,
    /// Polecenie użytkownika (źródło).
    pub text: String,
    /// Reguły, które przeszły sprawdzenie (tylko zawężają).
    pub rules: Vec<Rule>,
    /// Szkice odrzucone (np. rozszerzające) — nigdy nie wejdą w życie.
    pub rejected: Vec<RejectedDraft>,
    /// Konflikty (z regułami aktywnymi i wewnątrz propozycji).
    pub conflicts: Vec<Conflict>,
    /// Stan.
    pub status: ProposalStatus,
    /// Kiedy (ms).
    pub created_at_ms: u64,
}

/// Błąd Marszałka.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "error", rename_all = "snake_case")]
pub enum MarshalError {
    /// Tylko użytkownik zatwierdza i cofa reguły.
    #[error("reguły zatwierdza i cofa wyłącznie użytkownik")]
    Forbidden,
    /// Nieznana propozycja.
    #[error("nieznana propozycja {0}")]
    UnknownProposal(u64),
    /// Nieznana reguła.
    #[error("nieznana reguła {0}")]
    UnknownRule(RuleId),
    /// Propozycja już rozstrzygnięta.
    #[error("propozycja {0} już rozstrzygnięta")]
    Decided(u64),
    /// Propozycja bez reguł do zatwierdzenia.
    #[error("propozycja {0} nie zawiera poprawnych reguł")]
    Empty(u64),
    /// Tłumacz (LLM) zawiódł.
    #[error("tłumaczenie polecenia: {0}")]
    Translator(String),
    /// Moduł nie działa.
    #[error("Marszałek nie jest uruchomiony")]
    NotStarted,
}

/// Stan księgi.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct RuleBook {
    ceiling: Ceiling,
    active: BTreeMap<RuleId, Rule>,
    proposals: BTreeMap<u64, Proposal>,
    next: u64,
}

fn violation_text(v: &Violation) -> String {
    match v {
        Violation::Widening { effect, detail } => {
            format!("rozszerza uprawnienia ({effect}): {detail}")
        }
        Violation::Invalid { detail } => detail.clone(),
    }
}

impl RuleBook {
    /// Pusta księga z sufitem.
    pub fn new(ceiling: Ceiling) -> Self {
        Self {
            ceiling,
            ..Self::default()
        }
    }

    /// Sufit.
    pub fn ceiling(&self) -> &Ceiling {
        &self.ceiling
    }

    /// Nowy sufit (zmiana uprawnień przez użytkownika/Broker); reguły, które przestały
    /// zawężać, nie działają (polityka efektywna je pomija).
    pub fn set_ceiling(&mut self, ceiling: Ceiling) {
        self.ceiling = ceiling;
    }

    /// Propozycja ze szkiców (JSON): ścisłe parsowanie + sprawdzenie względem sufitu.
    pub fn propose(&mut self, text: &str, drafts: Vec<serde_json::Value>, now_ms: u64) -> Proposal {
        let mut rules = Vec::new();
        let mut rejected = Vec::new();
        for draft in drafts {
            let errors = match parse_rule(&draft) {
                Ok(rule) => {
                    let violations = check_rule(&rule, &self.ceiling);
                    if violations.is_empty() && !rules.iter().any(|r: &Rule| r.id == rule.id) {
                        rules.push(rule);
                        continue;
                    }
                    if violations.is_empty() {
                        vec!["powtórzony identyfikator reguły".to_owned()]
                    } else {
                        violations.iter().map(violation_text).collect()
                    }
                }
                Err(e) => vec![e],
            };
            rejected.push(RejectedDraft { draft, errors });
        }
        let mut all: Vec<Rule> = self.active.values().cloned().collect();
        all.extend(rules.iter().cloned());
        let new_ids: Vec<&RuleId> = rules.iter().map(|r| &r.id).collect();
        let found = conflicts(&all)
            .into_iter()
            .filter(|c| new_ids.contains(&&c.first) || new_ids.contains(&&c.second))
            .collect();
        self.next += 1;
        let proposal = Proposal {
            id: self.next,
            text: text.chars().take(2_000).collect(),
            rules,
            rejected,
            conflicts: found,
            status: ProposalStatus::Pending,
            created_at_ms: now_ms,
        };
        self.proposals.insert(proposal.id, proposal.clone());
        proposal
    }

    /// Zatwierdzenie (tylko użytkownik): reguły stają się aktywne (zastępują te o tym samym id).
    pub fn approve(&mut self, id: u64, approver: &Approver) -> Result<Vec<Rule>, MarshalError> {
        if !approver.is_user() {
            return Err(MarshalError::Forbidden);
        }
        let p = self
            .proposals
            .get_mut(&id)
            .ok_or(MarshalError::UnknownProposal(id))?;
        if p.status != ProposalStatus::Pending {
            return Err(MarshalError::Decided(id));
        }
        if p.rules.is_empty() {
            return Err(MarshalError::Empty(id));
        }
        // Ponowne sprawdzenie (sufit mógł zmaleć od propozycji).
        let ok: Vec<Rule> = p
            .rules
            .iter()
            .filter(|r| check_rule(r, &self.ceiling).is_empty())
            .cloned()
            .collect();
        if ok.is_empty() {
            return Err(MarshalError::Empty(id));
        }
        p.status = ProposalStatus::Approved;
        for rule in &ok {
            self.active.insert(rule.id.clone(), rule.clone());
        }
        Ok(ok)
    }

    /// Odrzucenie (użytkownik albo Marszałek na polecenie — odrzucenie nigdy nie rozszerza).
    pub fn reject(&mut self, id: u64) -> Result<(), MarshalError> {
        let p = self
            .proposals
            .get_mut(&id)
            .ok_or(MarshalError::UnknownProposal(id))?;
        if p.status != ProposalStatus::Pending {
            return Err(MarshalError::Decided(id));
        }
        p.status = ProposalStatus::Rejected;
        Ok(())
    }

    /// Cofnięcie aktywnej reguły — przywraca szersze zachowanie, więc tylko użytkownik.
    pub fn revoke(&mut self, rule: &RuleId, approver: &Approver) -> Result<Rule, MarshalError> {
        if !approver.is_user() {
            return Err(MarshalError::Forbidden);
        }
        self.active
            .remove(rule)
            .ok_or_else(|| MarshalError::UnknownRule(rule.clone()))
    }

    /// Reguły aktywne.
    pub fn active(&self) -> Vec<Rule> {
        self.active.values().cloned().collect()
    }

    /// Propozycja.
    pub fn proposal(&self, id: u64) -> Option<Proposal> {
        self.proposals.get(&id).cloned()
    }

    /// Polityka efektywna (sufit ∩ aktywne reguły).
    pub fn effective(&self) -> EffectivePolicy {
        compose(&self.ceiling, &self.active())
    }
}
