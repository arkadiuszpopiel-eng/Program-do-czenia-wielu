//! Polityka Kreatora (PLAN §8.1 Jądro, §8.3 autonomia, §12.1): **lista dozwolonych** grup
//! narzędzi (nic spoza niej; znaczniki Jądra odrzucane osobno), sufit autonomii = poziom
//! tworzącej sesji, nigdy L4 (L4 tylko przełącznikiem w Ustawieniach), sufit budżetu, zakresy
//! zapisu wyłącznie w profilu użytkownika (bez katalogów Alfy, Jądra, systemu i poświadczeń),
//! prompty bez poleceń obejścia zabezpieczeń i w rodzaju żeńskim.

use std::collections::BTreeSet;

use agent_runtime_contract::RunBudget;
use personas_contract::{builtin_roles, fold};
use risk_classifier_contract::AutonomyLevel;

/// Błędy Kreatora (komunikaty po polsku — trafiają do rozmowy i formularza).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum BuildError {
    /// Brak wymaganego pola.
    #[error("brak pola: {0}")]
    Missing(String),
    /// Niepoprawne pole.
    #[error("niepoprawne pole {0}")]
    Invalid(String),
    /// Uprawnienia Jądra (Broker, audyt, autonomia, polityki, sekrety, administracja).
    #[error("Kreator nie tworzy ról z uprawnieniami Jądra: {0}")]
    KernelPermission(String),
    /// Grupa narzędzi spoza listy dozwolonych.
    #[error("grupa narzędzi `{0}` nie jest dozwolona w Kreatorze")]
    ForbiddenGroup(String),
    /// Autonomia ponad sufit.
    #[error(
        "poziom autonomii {requested:?} ponad dozwolony {ceiling:?} — Kreator nie podnosi autonomii"
    )]
    AutonomyTooHigh {
        /// Żądany.
        requested: AutonomyLevel,
        /// Sufit.
        ceiling: AutonomyLevel,
    },
    /// Budżet ponad sufit.
    #[error("budżet ponad sufit Kreatora")]
    BudgetTooHigh,
    /// Zakres zapisu poza profilem albo na liście zakazanych.
    #[error("zakres zapisu niedozwolony: {0}")]
    ForbiddenPath(String),
    /// Prompt z poleceniem obejścia zabezpieczeń.
    #[error("prompt zawiera niedozwolone polecenie: „{0}”")]
    PromptPolicy(String),
    /// Forma męska w prompcie.
    #[error("prompt musi być w rodzaju żeńskim (znaleziono „{0}”)")]
    Masculine(String),
    /// Kolizja z istniejącą personą, rolą albo głosem.
    #[error("kolizja: {0}")]
    Conflict(String),
    /// Zapis bez udanego testu na sucho tego samego manifestu.
    #[error("najpierw test na sucho tego manifestu")]
    DryRunRequired,
    /// Zatwierdzenie dotyczy innej treści albo złym kanałem.
    #[error("zatwierdzenie niezgodne: {0}")]
    Approval(String),
    /// Magazyn albo usługa person.
    #[error("zapis: {0}")]
    Store(String),
}

/// Znaczniki uprawnień Jądra w identyfikatorach (grupy, role, narzędzia).
pub const KERNEL_MARKERS: [&str; 27] = [
    "kernel",
    "jadr",
    "broker",
    "audit",
    "audyt",
    "watchdog",
    "updater",
    "kill-switch",
    "killswitch",
    "policy",
    "polityk",
    "autonom",
    "admin",
    "secret",
    "sekret",
    "credential",
    "privacy",
    "prywatn",
    "egress",
    "deny",
    "taint",
    "approv",
    "zatwierdz",
    "windows-hello",
    "elevat",
    "token",
    "system.",
];

/// Frazy (po `fold`) poleceń obejścia zabezpieczeń w promptach i opisach.
pub const PROMPT_PHRASES: [&str; 29] = [
    "jadr",
    "podnies poziom",
    "podnies autonomi",
    "zmien poziom autonomii",
    "poziom maks",
    "wylacz audyt",
    "pomin audyt",
    "omijaj broker",
    "omin broker",
    "ignoruj broker",
    "bez brokera",
    "zatwierdzaj sama",
    "zatwierdz sama",
    "sama zatwierdz",
    "zmien uprawnienia",
    "nadaj sobie",
    "kill-switch",
    "kill switch",
    "wylacz watchdog",
    "deny-list",
    "zmien polityk",
    "haslo",
    "hasla",
    ".claude",
    ".codex",
    "ciasteczk",
    "cookies",
    "ignoruj poprzednie",
    "zignoruj zasady",
];

/// Słowa (po `fold`) zakazane jako osobne tokeny.
pub const PROMPT_TOKENS: [&str; 5] = ["l4", "sudo", "root", "admin", "administrator"];

/// Fragmenty zakazanych zakresów zapisu (po normalizacji: małe litery, `/`).
pub const FORBIDDEN_PATH_PARTS: [&str; 14] = [
    "%localappdata%/alfa",
    "appdata/local/alfa",
    "/alfa/broker",
    "/windows/",
    "system32",
    "program files",
    "programdata",
    "/.ssh",
    "/.claude",
    "/.codex",
    "/.gnupg",
    "microsoft/credentials",
    "microsoft/protect",
    "..",
];

/// Polityka Kreatora.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuilderPolicy {
    /// Dozwolone grupy narzędzi (domyślnie: grupy ról wbudowanych).
    pub allowed_groups: BTreeSet<String>,
    /// Sufit autonomii nowej agentki (≤ L3).
    pub ceiling: AutonomyLevel,
    /// Sufit budżetu.
    pub max_budget: RunBudget,
    /// Paleta tokenów kolorów dla własnych agentek.
    pub palette: Vec<String>,
}

impl Default for BuilderPolicy {
    fn default() -> Self {
        Self {
            allowed_groups: builtin_roles().into_iter().flat_map(|r| r.tools).collect(),
            ceiling: AutonomyLevel::L3,
            max_budget: RunBudget {
                max_steps: 200,
                max_tokens: 2_000_000,
                max_wall_ms: 2 * 60 * 60 * 1000,
                max_cost_micro_usd: None,
                max_tool_calls_per_turn: 8,
            },
            palette: (1..=8).map(|i| format!("color.agent.custom-{i}")).collect(),
        }
    }
}

impl BuilderPolicy {
    /// Polityka z sufitem = poziom tworzącej sesji (z Brokera), nigdy powyżej L3.
    pub fn with_ceiling(level: AutonomyLevel) -> Self {
        Self {
            ceiling: level.min(AutonomyLevel::L3),
            ..Self::default()
        }
    }

    /// Grupa narzędzi: ASCII `[a-z][a-z0-9.-]*`, bez znaczników Jądra, z listy dozwolonych.
    pub fn check_group(&self, group: &str) -> Result<(), BuildError> {
        let ok_chars = group.chars().next().is_some_and(|c| c.is_ascii_lowercase())
            && group.len() <= 32
            && group
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '.' || c == '-');
        if !ok_chars {
            return Err(BuildError::ForbiddenGroup(group.to_owned()));
        }
        check_kernel_id(group)?;
        if !self.allowed_groups.contains(group) {
            return Err(BuildError::ForbiddenGroup(group.to_owned()));
        }
        Ok(())
    }

    /// Poziom autonomii ≤ sufit (i nigdy L4).
    pub fn check_autonomy(&self, requested: AutonomyLevel) -> Result<(), BuildError> {
        let ceiling = self.ceiling.min(AutonomyLevel::L3);
        if requested > ceiling {
            return Err(BuildError::AutonomyTooHigh { requested, ceiling });
        }
        Ok(())
    }
}

/// Identyfikator bez znaczników Jądra (po `fold`, małe litery).
pub fn check_kernel_id(id: &str) -> Result<(), BuildError> {
    let f = fold(id);
    match KERNEL_MARKERS.iter().find(|m| f.contains(*m)) {
        Some(_) => Err(BuildError::KernelPermission(id.to_owned())),
        None => Ok(()),
    }
}

/// Tekst (prompt, opis, charakter) bez poleceń obejścia zabezpieczeń.
pub fn check_text(text: &str) -> Result<(), BuildError> {
    let f = fold(text);
    if let Some(p) = PROMPT_PHRASES.iter().find(|p| f.contains(*p)) {
        return Err(BuildError::PromptPolicy((*p).to_owned()));
    }
    let tokens: Vec<&str> = f.split(|c: char| !c.is_alphanumeric()).collect();
    if let Some(t) = PROMPT_TOKENS.iter().find(|t| tokens.contains(t)) {
        return Err(BuildError::PromptPolicy((*t).to_owned()));
    }
    Ok(())
}

/// Formy męskie (czasowniki 1./2. os. przeszłe, przymiotniki) — prompt musi być żeński.
pub fn check_feminine(text: &str) -> Result<(), BuildError> {
    let bad = text.split(|c: char| !c.is_alphabetic()).find(|w| {
        let w = w.to_lowercase();
        w.ends_with("łem")
            || w.ends_with("łeś")
            || matches!(
                w.as_str(),
                "gotowy" | "pewny" | "sam" | "zadowolony" | "powinienem"
            )
    });
    match bad {
        Some(w) => Err(BuildError::Masculine(w.to_owned())),
        None => Ok(()),
    }
}

/// Zakres zapisu: w profilu użytkownika (`%USERPROFILE%`, `~`, `C:\Users\<konto>`), bez
/// fragmentów zakazanych, nie cały dysk.
pub fn check_fs_scope(scope: &str) -> Result<(), BuildError> {
    let n = scope.trim().replace('\\', "/").to_lowercase();
    let forbidden = || BuildError::ForbiddenPath(scope.to_owned());
    if n.is_empty() || n.starts_with("//") || n.contains('\0') {
        return Err(forbidden());
    }
    let in_profile = n.starts_with("%userprofile%/")
        || n.starts_with("~/")
        || n.strip_prefix("c:/users/").is_some_and(|rest| {
            rest.split('/')
                .next()
                .is_some_and(|u| !u.is_empty() && u != "*" && u != "**" && u != "public")
        });
    if !in_profile || FORBIDDEN_PATH_PARTS.iter().any(|p| n.contains(p)) {
        return Err(forbidden());
    }
    Ok(())
}
