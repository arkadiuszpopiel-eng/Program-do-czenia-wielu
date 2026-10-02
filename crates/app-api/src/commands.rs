//! Lista komend IPC (COMMANDS.md) — jedno źródło dla powłoki Tauri (`with_commands!` generuje
//! handlery `#[tauri::command]`, reeksport w `app-core`) i testów (`app-core/tests/ipc_signatures.rs`
//! sprawdza sygnatury `AppCore` i to, że przyszłości są `Send + 'static`, czego wymaga Tauri).
//! Komendy ze strumieniem w kanale `tauri::ipc::Channel` ([`CHANNEL_COMMANDS`]) nie mieszczą się
//! w tej liście (argument kanału istnieje tylko w powłoce) — powłoka obsługuje je ręcznie.

/// Wywołuje makro `$callback` z listą komend w postaci `nazwa(arg: Typ, …) -> Wynik;`.
#[macro_export]
macro_rules! with_commands {
    ($callback:ident) => {
        $callback! {
            app_bootstrap() -> $crate::dto::AppBootstrap;
            app_complete_onboarding() -> ();
            app_open_system_settings(uri: String) -> ();
            app_save_layout(layout: $crate::dto::LayoutPrefs) -> ();
            app_set_active_session(session_id: Option<String>) -> ();
            sessions_list() -> Vec<$crate::dto::SessionSummary>;
            sessions_create(template: $crate::dto::SessionTemplate) -> $crate::dto::SessionSummary;
            sessions_rename(session_id: String, title: String) -> ();
            sessions_set_pinned(session_id: String, pinned: bool) -> ();
            sessions_set_project(session_id: String, project: Option<String>) -> ();
            sessions_set_archived(session_id: String, archived: bool) -> ();
            sessions_remove(session_id: String) -> $crate::dto::UndoTicket;
            sessions_undo_remove(token: String) -> ();
            sessions_duplicate_as_template(session_id: String) -> $crate::dto::SessionSummary;
            sessions_export(session_id: String) -> $crate::dto::ExportResult;
            sessions_search(query: String) -> Vec<$crate::dto::SessionSearchHit>;
            sessions_mark_read(session_id: String) -> ();
            sessions_get_draft(session_id: String) -> String;
            sessions_save_draft(session_id: String, text: String) -> ();
            sessions_workdir(session_id: String) -> $crate::dto::SessionWorkdir;
            sessions_choose_workdir(session_id: String, choice: $crate::dto::WorkdirChoice) -> $crate::dto::SessionWorkdir;
            turns_list(session_id: String) -> $crate::dto::TurnsSnapshot;
            turns_send(session_id: String, options: $crate::dto::SendOptions) -> $crate::dto::SendResult;
            turns_regenerate(session_id: String, turn_id: String, profile: Option<String>) -> String;
            turns_edit_and_resend(session_id: String, turn_id: String, text: String) -> $crate::dto::SendResult;
            turns_continue(session_id: String, turn_id: String) -> String;
            turns_stop(session_id: String) -> ();
            turns_rate(turn_id: String, rating: Option<$crate::dto::Rating>) -> ();
            turns_set_hidden(turn_id: String, hidden: bool) -> ();
            turns_remember(turn_id: String, scope: $crate::dto::RememberScope) -> ();
            turns_read_aloud(turn_id: String) -> ();
            turns_save_code(turn_id: String, block_index: u64) -> ();
            turns_run_code(turn_id: String, block_index: u64) -> $crate::dto::BrokerIntentResult;
            turns_undo_step(undo_token: String) -> ();
            agents_list(session_id: String) -> Vec<$crate::dto::AgentState>;
            agents_set_roles(session_id: String, agent: String, role_ids: Vec<String>) -> ();
            agents_apply_cast(session_id: String, template: $crate::dto::CastTemplateId) -> ();
            agents_runs(session_id: String) -> Vec<$crate::dto::AgentRunDetail>;
            agents_steer(session_id: String, text: String) -> ();
            agents_open_terminal(step_id: String) -> ();
            costs_summary(session_id: Option<String>) -> $crate::dto::CostSummary;
            costs_set_monthly_limit(enabled: bool, monthly: $crate::dto::Money) -> ();
            settings_schema() -> Vec<$crate::dto::SettingsPageDef>;
            settings_values() -> std::collections::BTreeMap<String, $crate::dto::SettingValue>;
            settings_set(key: String, value: $crate::dto::SettingValue) -> ();
            settings_reset(key: String) -> $crate::dto::SettingValue;
            settings_set_shortcut(action_id: String, chord: Option<String>) -> ();
            timeline_list(session_id: String, filter: $crate::dto::TimelineFilter) -> Vec<$crate::dto::TimelineEvent>;
            files_list(session_id: String) -> Vec<$crate::dto::ArtifactInfo>;
            files_preview(artifact_id: String) -> $crate::dto::ArtifactPreview;
            files_act(artifact_id: String, action: $crate::dto::ArtifactAction) -> ();
            accounts_catalog() -> Vec<$crate::dto::ProviderInfo>;
            accounts_list() -> Vec<$crate::dto::Account>;
            accounts_add(input: $crate::dto::AddAccountInput) -> $crate::dto::Account;
            accounts_test(account_id: String) -> $crate::dto::TestReport;
            accounts_assign(account_id: String, assignment: $crate::dto::AccountAssignment) -> ();
            accounts_set_limit(account_id: String, enabled: bool, monthly: $crate::dto::Money) -> ();
            accounts_remove(account_id: String) -> ();
            transfer_export(request: $crate::dto::ExportRequest) -> $crate::dto::ExportResult;
            transfer_export_secrets(password: $crate::dto::SecretInput) -> $crate::dto::ExportResult;
            transfer_inspect(password: Option<$crate::dto::SecretInput>, path: Option<String>) -> $crate::dto::InspectResult;
            transfer_import(request: $crate::dto::ImportRequest) -> $crate::dto::ImportResult;
            transfer_rollback(snapshot_id: String) -> ();
            permissions_get(session_id: Option<String>) -> $crate::dto::PermissionsState;
            permissions_request_level(level: $crate::dto::AutonomyLevel, session_id: Option<String>) -> $crate::dto::BrokerIntentResult;
            permissions_open_approval(approval_id: String) -> $crate::dto::BrokerIntentResult;
            models_local_list() -> Vec<$crate::dto::LocalModelInfo>;
            models_local_download(model_id: Option<String>) -> ();
            models_local_cancel(model_id: Option<String>) -> ();
            device_profile() -> $crate::dto::DeviceProfile;
            device_measure() -> $crate::dto::DeviceProfile;
            voice_devices() -> Vec<$crate::dto::AudioDevice>;
            voice_start_mic_test(device_id: Option<String>) -> ();
            voice_stop_mic_test() -> ();
            voice_set_mic_enabled(enabled: bool) -> ();
            voice_set_muted(muted: bool) -> ();
            voice_stop_speech() -> ();
            voice_status() -> $crate::dto::VoiceStatus;
            voice_ptt(pressed: bool) -> ();
            voice_preview(agent: String) -> ();
            system_status() -> $crate::dto::SystemStatus;
            system_retry_queue() -> ();
            quick_ask(text: String) -> $crate::dto::QuickAskResult;
            quick_expand_to_main(session_id: String) -> ();
            quick_hide() -> ();
            memory_status() -> $crate::dto::MemoryStatus;
            memory_scopes() -> Vec<$crate::dto::MemoryScopeInfo>;
            memory_inspect(query: $crate::dto::MemoryQuery) -> $crate::dto::MemoryPage;
            memory_explain(entry_id: String) -> $crate::dto::MemoryExplanation;
            memory_edit(entry_id: String, edit: $crate::dto::MemoryEdit) -> $crate::dto::MemoryItem;
            memory_set_pinned(entry_id: String, pinned: bool) -> $crate::dto::MemoryItem;
            memory_approve(entry_id: String) -> $crate::dto::MemoryItem;
            memory_promote(entry_id: String, to: $crate::dto::MemoryScopeRef) -> $crate::dto::MemoryItem;
            memory_forget_preview(target: $crate::dto::MemoryForgetTarget) -> $crate::dto::MemoryForgetPreview;
            memory_forget(target: $crate::dto::MemoryForgetTarget) -> $crate::dto::MemoryForgetReport;
            memory_journal(scope: String) -> Vec<$crate::dto::MemoryJournalEntry>;
            memory_undo(scope: String, change_id: String) -> $crate::dto::MemoryUndoResult;
            memory_consolidate_now() -> $crate::dto::ConsolidationReport;
            tasks_list() -> Vec<$crate::dto::TaskInfo>;
            tasks_create(input: $crate::dto::NewTaskInput) -> $crate::dto::TaskInfo;
            tasks_cancel(task_id: String) -> Vec<String>;
            tasks_retry(task_id: String) -> $crate::dto::TaskInfo;
            tasks_steer(task_id: String, text: String) -> ();
            tasks_pause(task_id: String) -> ();
            tasks_resume(task_id: String) -> ();
            triggers_list() -> Vec<$crate::dto::TriggerInfo>;
            triggers_create(draft: $crate::dto::TriggerDraft) -> $crate::dto::TriggerInfo;
            triggers_remove(trigger_id: String) -> ();
            triggers_set_enabled(trigger_id: String, enabled: bool) -> ();
            triggers_fire_now(trigger_id: String) -> $crate::dto::TriggerRunInfo;
            triggers_log(trigger_id: Option<String>) -> Vec<$crate::dto::TriggerRunInfo>;
            triggers_preview_cron(expr: String) -> $crate::dto::CronPreview;
            marshal_state() -> $crate::dto::MarshalState;
            marshal_propose(text: String, drafts: Option<Vec<serde_json::Value>>) -> $crate::dto::MarshalProposalInfo;
            marshal_approve(proposal_id: u64) -> Vec<$crate::dto::MarshalRuleInfo>;
            marshal_reject(proposal_id: u64) -> ();
            marshal_revoke(rule_id: String) -> ();
            marshal_report() -> $crate::dto::MarshalReport;
            bridges_list(refresh: bool) -> Vec<$crate::dto::BridgeCard>;
            bridges_set_enabled(route_id: String, enabled: bool) -> $crate::dto::BridgeCard;
            bridges_set_schedule(bridge: String, per_day: u32) -> $crate::dto::BridgeCard;
            bridges_pin(bridge: String, version: Option<String>) -> $crate::dto::BridgeCard;
            bridges_open_login(bridge: String) -> $crate::dto::BridgeLogin;
            gui_status() -> $crate::dto::GuiStatus;
            gui_screenshot() -> Option<$crate::dto::GuiScreenshot>;
            gui_stop() -> $crate::dto::GuiStatus;
            gui_release() -> $crate::dto::GuiStatus;
            gui_desktop_grant(session_id: String, agent: String) -> $crate::dto::BrokerIntentResult;
            terminal_input(terminal: u64, data_b64: String) -> ();
            terminal_resize(terminal: u64, cols: u16, rows: u16) -> ();
            terminal_close(terminal: u64) -> ();
            terminal_list() -> Vec<$crate::dto::TerminalSession>;
            skills_list() -> Vec<$crate::dto::SkillInfo>;
            skills_review(skill_id: String, version: String) -> $crate::dto::SkillReview;
            skills_propose(skill: serde_json::Value) -> $crate::dto::SkillInfo;
            skills_approve(skill_id: String, version: String, hash: String) -> $crate::dto::SkillInfo;
            skills_release(skill_id: String, version: String, hash: String) -> $crate::dto::SkillInfo;
            skills_reject(skill_id: String, version: String) -> $crate::dto::SkillInfo;
            skills_disable(skill_id: String) -> $crate::dto::SkillInfo;
            skills_run(skill_id: String, session_id: String, agent: Option<String>, params: serde_json::Value) -> $crate::dto::TaskInfo;
            skills_export() -> $crate::dto::ExportResult;
            skills_import() -> $crate::dto::SkillImportResult;
            builder_policy() -> $crate::dto::BuilderPolicyView;
            builder_propose(description: String) -> $crate::dto::BuilderProposal;
            builder_preview(draft: $crate::dto::AgentDraft) -> $crate::dto::BuilderPreview;
            builder_dry_run(draft: $crate::dto::AgentDraft) -> $crate::dto::BuilderDryRun;
            builder_save(draft: $crate::dto::AgentDraft, hash: String) -> $crate::dto::BuilderSaved;
            builder_voice_preview(draft: $crate::dto::AgentDraft) -> ();
            builder_library() -> Vec<$crate::dto::BuilderAgentInfo>;
            health_report() -> $crate::dto::HealthView;
            health_scan() -> $crate::dto::HealthView;
            health_approve(repair_id: u64) -> $crate::dto::HealthView;
            health_reject(repair_id: u64) -> $crate::dto::HealthView;
            health_undo(repair_id: u64) -> $crate::dto::HealthView;
            improver_list() -> $crate::dto::ImproverView;
            improver_cycle() -> $crate::dto::ImproverView;
            improver_approve(proposal_id: u64, digest: String) -> $crate::dto::ImproverView;
            improver_reject(proposal_id: u64) -> $crate::dto::ImproverView;
            improver_rollback(proposal_id: u64) -> $crate::dto::ImproverView;
            evals_list() -> $crate::dto::EvalsView;
            evals_verify(suite_id: String) -> $crate::dto::EvalSuiteView;
        }
    };
}

/// Komendy ze strumieniem w `tauri::ipc::Channel` (handler w powłoce; metoda `AppCore` dostaje
/// odbiorcę strumienia zamiast kanału): `terminal_open` — wyjście VT terminala.
pub const CHANNEL_COMMANDS: &[&str] = &["terminal_open"];

macro_rules! command_names {
    ($( $name:ident ( $( $arg:ident : $ty:ty ),* ) -> $ret:ty ; )*) => {
        /// Wszystkie komendy wystawiane przez `AppCore` (nazwa = komenda Tauri = metoda):
        /// lista `with_commands!` + [`CHANNEL_COMMANDS`].
        pub const COMMANDS: &[&str] = &[$( stringify!($name), )* "terminal_open"];
    };
}

with_commands!(command_names);

#[cfg(test)]
mod tests {
    #[test]
    fn channel_commands_are_listed_once() {
        for c in super::CHANNEL_COMMANDS {
            assert_eq!(super::COMMANDS.iter().filter(|x| *x == c).count(), 1);
        }
        assert_eq!(super::COMMANDS.len(), 162);
    }
}
