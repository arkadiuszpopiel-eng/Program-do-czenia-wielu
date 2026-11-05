//! Model-sędzia trybu „Automatycznie” po stronie Brokera (plan 50 §4.3, Z.4,
//! `docs/modules/approval-judge/SPEC.md`).
//!
//! Broker nie woła modeli: prośbę kwalifikującą się do sędziego zostawia w stanie „czeka na sędziego”,
//! a kierowca w rdzeniu (rola IPC `Core`) pobiera ją przez `judge_queue` i odsyła werdykt przez
//! `judge_resolve`. Sędzia daje co najwyżej zgodę jednorazową; po terminie albo przy błędzie prośba trafia
//! na kartę właściciela.

use approval_judge_contract::{FuseLimits, JudgeVerdict};
use risk_classifier_contract::{Destructiveness, Reversibility, RuleId};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::approval::ApprovalId;
use crate::capability::Capability;
use crate::token::Holder;

/// Audyt: prośba oddana sędziemu.
pub const EVENT_JUDGE_REQUESTED: &str = "broker.judge.requested";
/// Audyt: werdykt sędziego (werdykt, kategoria, powód, model).
pub const EVENT_JUDGE_DECIDED: &str = "broker.judge.decided";
/// Audyt: sędzia nie zdążył — prośba przeszła na kartę właściciela.
pub const EVENT_JUDGE_TIMEOUT: &str = "broker.judge.timeout";
/// Audyt: bezpiecznik otwarty (seria odmów) albo zamknięty zgodą właściciela.
pub const EVENT_JUDGE_FUSE: &str = "broker.judge.fuse";

/// Ustawienia sędziego w polityce Jądra (`[security.judge]`). Zmienia je właściciel jak resztę polityki.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct JudgePolicy {
    /// Czy tryb „Automatycznie” oddaje prośby sędziemu.
    pub enabled: bool,
    /// Ile sędzia ma na werdykt (ms, 5–120 s); potem karta właściciela.
    pub timeout_ms: u64,
    /// Odmowy z rzędu otwierające bezpiecznik.
    pub fuse_consecutive: u32,
    /// Odmowy w zadaniu otwierające bezpiecznik.
    pub fuse_per_task: u32,
}

/// Kierowca, który nie pytał o kolejkę dłużej niż tyle, uznawany jest za nieobecnego — prośby idą wtedy
/// od razu do właściciela (bez czekania na termin sędziego).
pub const JUDGE_DRIVER_STALE_MS: u64 = 5_000;

impl Default for JudgePolicy {
    fn default() -> Self {
        Self {
            enabled: true,
            timeout_ms: 30_000,
            fuse_consecutive: 3,
            fuse_per_task: 20,
        }
    }
}

impl JudgePolicy {
    /// Progi bezpiecznika.
    pub fn fuse_limits(&self) -> FuseLimits {
        FuseLimits {
            consecutive: self.fuse_consecutive,
            per_task: self.fuse_per_task,
        }
    }

    /// Walidacja zakresów.
    pub fn validate(&self) -> Result<(), String> {
        if !(5_000..=120_000).contains(&self.timeout_ms) {
            return Err("judge.timeout_ms musi być w [5 s, 120 s]".into());
        }
        if self.fuse_consecutive == 0 || self.fuse_per_task == 0 {
            return Err("progi bezpiecznika sędziego muszą być ≥ 1".into());
        }
        Ok(())
    }
}

/// Prośba czekająca na sędziego (fakty Sędziego Cieni, bez tokenów i nonce).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct JudgeRequest {
    /// Prośba.
    pub approval: ApprovalId,
    /// Proszący (sesja i agentka — kierowca bierze z nich zapis rozmowy).
    pub holder: Holder,
    /// Narzędzie.
    pub tool: String,
    /// Zdolność.
    pub capability: Capability,
    /// Opis akcji od narzędzia.
    pub summary: Option<String>,
    /// Polecenie powłoki.
    pub command: Option<String>,
    /// Folder pracy rozmowy.
    pub workdir: Option<String>,
    /// Odwracalność.
    pub reversible: Reversibility,
    /// Destrukcyjność.
    pub destructive: Destructiveness,
    /// Reguły, przez które Sędzia Cieni pyta.
    pub rules: Vec<RuleId>,
    /// Sesja widziała treść obcą.
    pub tainted: bool,
    /// Zmiana poza folderem pracy.
    pub outside_workdir: bool,
    /// Host wyjścia sieciowego.
    pub host: Option<String>,
    /// Termin werdyktu (ms) — później prośba przechodzi na kartę właściciela.
    pub deadline_ms: u64,
}

/// Werdykt odsyłany przez kierowcę.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct JudgeResolution {
    /// Werdykt modelu (`Allow` = tylko ta jedna akcja).
    pub verdict: JudgeVerdict,
    /// Model (dziennik).
    pub model: String,
    /// Zadanie (przebieg główny rozmowy); zmiana zeruje licznik bezpiecznika „w zadaniu”.
    pub task: Option<String>,
    /// Czas oceny (ms, dziennik).
    pub took_ms: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_policy_is_valid() {
        assert_eq!(JudgePolicy::default().validate(), Ok(()));
        let bad = JudgePolicy {
            timeout_ms: 1,
            ..JudgePolicy::default()
        };
        assert!(bad.validate().is_err());
    }

    #[test]
    fn missing_policy_section_means_defaults() {
        let p: JudgePolicy = serde_json::from_str("{}").unwrap();
        assert_eq!(p, JudgePolicy::default());
    }
}
