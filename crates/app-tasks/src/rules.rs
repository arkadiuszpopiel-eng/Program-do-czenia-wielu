//! Komendy Reguł Marszałka (`marshal_*`): polecenie → szkice → podgląd zawężenia → zatwierdzenie
//! wyłącznie przez użytkownika w UI; egzekucja polityki po stronie aplikacji (tylko zawężenia).

use std::sync::Arc;
use std::sync::atomic::Ordering;

use app_api::AppError;
use app_api::dto::{MarshalProposalInfo, MarshalReport, MarshalRuleInfo, MarshalState};
use marshal_contract::{Approver, Marshal, ProposalStatus, RuleId};
use marshal_impl::MarshalModule;

use crate::app::{TasksApp, err, unavailable};
use crate::map;

impl TasksApp {
    fn marshal(&self) -> Result<&Arc<MarshalModule>, AppError> {
        self.p
            .marshal
            .as_ref()
            .ok_or_else(|| unavailable("Reguły Marszałka", "marshal"))
    }

    /// `marshal_state`.
    pub fn marshal_state(&self) -> Result<MarshalState, AppError> {
        let m = self.marshal()?;
        Ok(MarshalState {
            rules: m.rules().iter().map(map::rule).collect(),
            proposals: self.proposals().values().rev().map(map::proposal).collect(),
            effective: crate::text::policy(&m.effective()),
            translator: self.p.translator,
        })
    }

    /// `marshal_propose`: polecenie → szkice (model albo z edytora) → podgląd zawężenia.
    pub async fn marshal_propose(
        &self,
        text: &str,
        drafts: Option<Vec<serde_json::Value>>,
    ) -> Result<MarshalProposalInfo, AppError> {
        let m = self.marshal()?;
        let proposal = match drafts {
            Some(d) => m.propose_drafts(text, d),
            None => m.propose(text).await.map_err(err)?,
        };
        self.proposals().insert(proposal.id, proposal.clone());
        Ok(map::proposal(&proposal))
    }

    fn set_status(&self, id: u64, status: ProposalStatus) {
        if let Some(p) = self.proposals().get_mut(&id) {
            p.status = status;
        }
    }

    /// `marshal_approve` (wyłącznie użytkownik w UI).
    pub fn marshal_approve(&self, id: u64) -> Result<Vec<MarshalRuleInfo>, AppError> {
        let rules = self
            .marshal()?
            .approve(id, Approver::UserInterface)
            .map_err(err)?;
        self.set_status(id, ProposalStatus::Approved);
        self.apply_policy();
        Ok(rules.iter().map(map::rule).collect())
    }

    /// `marshal_reject`.
    pub fn marshal_reject(&self, id: u64) -> Result<(), AppError> {
        self.marshal()?.reject(id).map_err(err)?;
        self.set_status(id, ProposalStatus::Rejected);
        Ok(())
    }

    /// `marshal_revoke` (cofnięcie reguły przez użytkownika).
    pub fn marshal_revoke(&self, rule: &str) -> Result<(), AppError> {
        self.marshal()?
            .revoke(&RuleId::from(rule), Approver::UserInterface)
            .map_err(err)?;
        self.apply_policy();
        Ok(())
    }

    /// `marshal_report`: raport dnia (dzisiaj, czas lokalny).
    pub fn marshal_report(&self) -> Result<MarshalReport, AppError> {
        let today = chrono::Local::now().date_naive();
        Ok(map::report(&self.marshal()?.daily_report(today)))
    }

    /// Egzekucja polityki Marszałka po stronie aplikacji (tylko zawężenia): mosty zabronione,
    /// limit równoległości w obsadzie schedulera (tokeny zawęża Broker — poza zakresem `app-*`).
    pub fn apply_policy(&self) {
        let Some(m) = &self.p.marshal else {
            return;
        };
        let policy = m.effective();
        self.p
            .bridges_denied
            .store(policy.bridges_denied, Ordering::SeqCst);
        self.p.roster.set_policy(policy.max_parallel);
        self.p.roster.apply(self.p.scheduler.as_ref());
    }
}
