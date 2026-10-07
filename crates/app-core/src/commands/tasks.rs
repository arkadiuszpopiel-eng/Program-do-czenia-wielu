//! Komendy panelu Zadania (`tasks_*`), Wyzwalaczy (`triggers_*`) i Reguł Marszałka
//! (`marshal_*`) — delegują do `app-tasks::TasksApp` (działania jako użytkownik w UI).

use crate::core::AppCore;
use crate::dto::{
    CronPreview, MarshalProposalInfo, MarshalReport, MarshalRuleInfo, MarshalState, NewTaskInput,
    TaskInfo, TriggerDraft, TriggerInfo, TriggerRunInfo,
};
use crate::error::AppError;

impl AppCore {
    /// `tasks_list`: zadania (DAG) ze stanem, postępem i kosztem.
    pub async fn tasks_list(&self) -> Result<Vec<TaskInfo>, AppError> {
        Ok(self.inner.tasks.tasks())
    }

    /// `tasks_create`: zadanie od użytkownika (opcjonalnie po innych — DAG).
    pub async fn tasks_create(&self, input: NewTaskInput) -> Result<TaskInfo, AppError> {
        if let Some(s) = &input.session_id {
            self.ensure_session(&crate::ids::session(s)?)?;
        }
        self.inner.tasks.create(input)
    }

    /// `tasks_cancel`: anulowanie z poddrzewem; zwraca anulowane zadania.
    pub async fn tasks_cancel(&self, task_id: String) -> Result<Vec<String>, AppError> {
        self.inner.tasks.cancel(&task_id)
    }

    /// `tasks_retry`: nowe zadanie z tą samą specyfikacją i pochodzeniem.
    pub async fn tasks_retry(&self, task_id: String) -> Result<TaskInfo, AppError> {
        self.inner.tasks.retry(&task_id)
    }

    /// `tasks_steer`: wiadomość dla agentki w najbliższym punkcie atomowym.
    pub async fn tasks_steer(&self, task_id: String, text: String) -> Result<(), AppError> {
        self.inner.tasks.steer(&task_id, &text)
    }

    /// `tasks_pause`.
    pub async fn tasks_pause(&self, task_id: String) -> Result<(), AppError> {
        self.inner.tasks.pause(&task_id)
    }

    /// `tasks_resume`.
    pub async fn tasks_resume(&self, task_id: String) -> Result<(), AppError> {
        self.inner.tasks.resume(&task_id)
    }

    /// `triggers_list`.
    pub async fn triggers_list(&self) -> Result<Vec<TriggerInfo>, AppError> {
        Ok(self.inner.tasks.triggers_list())
    }

    /// `triggers_create`: wyzwalacz użytkownika (czas, zdarzenie, ręczny).
    pub async fn triggers_create(&self, draft: TriggerDraft) -> Result<TriggerInfo, AppError> {
        self.inner.tasks.trigger_create(&draft)
    }

    /// `triggers_remove`.
    pub async fn triggers_remove(&self, trigger_id: String) -> Result<(), AppError> {
        self.inner.tasks.trigger_remove(&trigger_id)
    }

    /// `triggers_set_enabled`.
    pub async fn triggers_set_enabled(
        &self,
        trigger_id: String,
        enabled: bool,
    ) -> Result<(), AppError> {
        self.inner.tasks.trigger_set_enabled(&trigger_id, enabled)
    }

    /// `triggers_fire_now`: „Uruchom teraz".
    pub async fn triggers_fire_now(&self, trigger_id: String) -> Result<TriggerRunInfo, AppError> {
        self.inner.tasks.trigger_fire(&trigger_id)
    }

    /// `triggers_log`: dziennik uruchomień (jednego albo wszystkich).
    pub async fn triggers_log(
        &self,
        trigger_id: Option<String>,
    ) -> Result<Vec<TriggerRunInfo>, AppError> {
        Ok(self.inner.tasks.trigger_log(trigger_id.as_deref()))
    }

    /// `triggers_preview_cron`: najbliższe uruchomienia wyrażenia (Europe/Warsaw).
    pub async fn triggers_preview_cron(&self, expr: String) -> Result<CronPreview, AppError> {
        Ok(app_tasks::TasksApp::preview_cron(&expr, 5))
    }

    /// `marshal_state`: reguły, propozycje, polityka obowiązująca.
    pub async fn marshal_state(&self) -> Result<MarshalState, AppError> {
        self.inner.tasks.marshal_state()
    }

    /// `marshal_propose`: polecenie → szkice (model albo edytor) → podgląd zawężenia.
    pub async fn marshal_propose(
        &self,
        text: String,
        drafts: Option<Vec<serde_json::Value>>,
    ) -> Result<MarshalProposalInfo, AppError> {
        self.inner.tasks.marshal_propose(&text, drafts).await
    }

    /// `marshal_approve`: zatwierdzenie propozycji (wyłącznie użytkownik w UI).
    pub async fn marshal_approve(
        &self,
        proposal_id: u64,
    ) -> Result<Vec<MarshalRuleInfo>, AppError> {
        self.inner.tasks.marshal_approve(proposal_id)
    }

    /// `marshal_reject`.
    pub async fn marshal_reject(&self, proposal_id: u64) -> Result<(), AppError> {
        self.inner.tasks.marshal_reject(proposal_id)
    }

    /// `marshal_revoke`: cofnięcie reguły.
    pub async fn marshal_revoke(&self, rule_id: String) -> Result<(), AppError> {
        self.inner.tasks.marshal_revoke(&rule_id)
    }

    /// `marshal_report`: raport dnia.
    pub async fn marshal_report(&self) -> Result<MarshalReport, AppError> {
        self.inner.tasks.marshal_report()
    }
}
