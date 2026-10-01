//! Polityki Jądra trzymane przez Brokera (`kernel_policy`): TTL, zakresy, allowlisty,
//! procesy i ścieżki chronione. Zmiana tylko przez Broker-UI z dowodem fizycznego wejścia;
//! elementów bazowych (procesy Jądra, obowiązkowe deny-listy) nie da się usunąć.

use compliance_contract::DenyLists;
use risk_classifier_contract::RiskPolicy;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::scope::{AppSelector, HostPattern, PathScope};

/// Procesy Jądra — zawsze chronione przed `gui.control` (THREAT_MODEL §7).
pub const PROTECTED_PROCESSES: [&str; 6] = [
    "alfa.exe",
    "alfa-core.exe",
    "alfa-broker.exe",
    "alfa-broker-ui.exe",
    "alfa-watchdog.exe",
    "alfa-uiaccess-helper.exe",
];

/// Usługi Windows Jądra i audytu systemowego — zatrzymanie/usunięcie = twarda blokada.
pub const PROTECTED_SERVICES: [&str; 3] = ["alfabroker", "alfawatchdog", "eventlog"];

/// Na czym wymagane jest Windows Hello (opcjonalnie, `hello.required_for`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum HelloRequirement {
    /// Podniesienie do L4.
    L4,
    /// Operacje administracyjne.
    Admin,
    /// Zmiany polityk Jądra.
    Policy,
}

/// Polityka Jądra.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct KernelPolicy {
    /// Domyślny TTL tokenu (ms; `token.ttl_default = "30m"`).
    pub token_ttl_default_ms: u64,
    /// Maksymalny TTL tokenu (ms).
    pub token_ttl_max_ms: u64,
    /// Ważność prośby o zatwierdzenie (ms).
    pub approval_ttl_ms: u64,
    /// Maksymalny czas „zawsze zezwalaj w tym zakresie” (ms).
    pub grant_max_ms: u64,
    /// Maksymalny czas planu po zatwierdzeniu (ms).
    pub plan_ttl_max_ms: u64,
    /// Zakres sesji (profil użytkownika) — w nim L3 działa sama.
    pub profile_roots: Vec<PathScope>,
    /// Wskazane aplikacje dla `gui.control`.
    pub allowed_apps: Vec<AppSelector>,
    /// Egress-allowlista.
    pub egress_allowlist: Vec<HostPattern>,
    /// Dodatkowe procesy chronione (bazowe z [`PROTECTED_PROCESSES`] zawsze obowiązują).
    pub extra_protected_processes: Vec<AppSelector>,
    /// Ścieżki Jądra (audyt, dane Brokera, polityki) — zapis = twarda blokada.
    pub kernel_paths: Vec<PathScope>,
    /// Litera dysku systemowego.
    pub system_drive: char,
    /// Deny-listy poświadczeń i webowych UI dostawców (z `compliance`).
    pub deny_lists: DenyLists,
    /// Progi klasyfikatora ryzyka.
    pub risk: RiskPolicy,
    /// Gdzie wymagane jest Windows Hello.
    pub hello_required_for: Vec<HelloRequirement>,
}

impl KernelPolicy {
    /// Polityka bazowa dla profilu `userprofile` (np. `C:\Users\ala`) i katalogu danych
    /// Brokera `broker_dir` (osobne konto usługi).
    pub fn baseline(userprofile: &str, broker_dir: &str) -> Result<Self, crate::scope::ScopeError> {
        use compliance_contract::PathEnv;
        let env = PathEnv::windows_profile(userprofile);
        let system_drive = userprofile
            .chars()
            .next()
            .filter(char::is_ascii_alphabetic)
            .map_or('c', |c| c.to_ascii_lowercase());
        Ok(Self {
            token_ttl_default_ms: 30 * 60 * 1000,
            token_ttl_max_ms: 4 * 60 * 60 * 1000,
            approval_ttl_ms: 10 * 60 * 1000,
            grant_max_ms: 24 * 60 * 60 * 1000,
            plan_ttl_max_ms: 2 * 60 * 60 * 1000,
            profile_roots: vec![PathScope::tree("%USERPROFILE%", &env)?],
            allowed_apps: Vec::new(),
            egress_allowlist: Vec::new(),
            extra_protected_processes: Vec::new(),
            kernel_paths: vec![
                PathScope::tree(broker_dir, &env)?,
                PathScope::tree(r"%APPDATA%\Alfa\kernel", &env)?,
                PathScope::tree(r"%LOCALAPPDATA%\Alfa\audit", &env)?,
            ],
            system_drive,
            deny_lists: DenyLists::baseline(),
            risk: RiskPolicy::default(),
            hello_required_for: Vec::new(),
        })
    }

    /// Walidacja: TTL skończone i dodatnie, obowiązkowe deny-listy, progi klasyfikatora.
    pub fn validate(&self) -> Result<(), String> {
        if self.token_ttl_default_ms == 0 || self.token_ttl_default_ms > self.token_ttl_max_ms {
            return Err("token_ttl_default_ms musi być w (0, token_ttl_max_ms]".into());
        }
        if self.token_ttl_max_ms > 24 * 60 * 60 * 1000 {
            return Err("token_ttl_max_ms nie może przekraczać 24 h".into());
        }
        if self.approval_ttl_ms == 0 || self.grant_max_ms == 0 || self.plan_ttl_max_ms == 0 {
            return Err("terminy zatwierdzeń muszą być dodatnie".into());
        }
        if !self.system_drive.is_ascii_lowercase() {
            return Err("system_drive musi być małą literą a–z".into());
        }
        self.deny_lists.validate().map_err(|e| e.to_string())?;
        self.risk.validate()
    }

    /// Czy aplikacja jest procesem chronionym (bazowym lub dodatkowym; także alias 8.3).
    pub fn is_protected_process(&self, app: &AppSelector) -> bool {
        use compliance_contract::deny::comp_matches;
        PROTECTED_PROCESSES
            .iter()
            .copied()
            .chain(self.extra_protected_processes.iter().map(AppSelector::exe))
            .any(|p| comp_matches(app.exe(), p))
    }
}
