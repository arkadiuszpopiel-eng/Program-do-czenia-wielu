//! Lista komend IPC (COMMANDS.md) — jedno źródło dla powłoki Tauri (`with_commands!` generuje
//! handlery `#[tauri::command]`) i testów (`tests/ipc_signatures.rs` sprawdza sygnatury `AppCore`
//! i to, że przyszłości są `Send + 'static`, czego wymaga Tauri).

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
            sessions_set_archived(session_id: String, archived: bool) -> ();
            sessions_remove(session_id: String) -> $crate::dto::UndoTicket;
            sessions_undo_remove(token: String) -> ();
            sessions_duplicate_as_template(session_id: String) -> $crate::dto::SessionSummary;
            sessions_export(session_id: String) -> $crate::dto::ExportResult;
            sessions_search(query: String) -> Vec<$crate::dto::SessionSearchHit>;
            sessions_mark_read(session_id: String) -> ();
            sessions_get_draft(session_id: String) -> String;
            sessions_save_draft(session_id: String, text: String) -> ();
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
            system_status() -> $crate::dto::SystemStatus;
            system_retry_queue() -> ();
            quick_ask(text: String) -> $crate::dto::QuickAskResult;
            quick_expand_to_main(session_id: String) -> ();
            quick_hide() -> ();
        }
    };
}

macro_rules! command_names {
    ($( $name:ident ( $( $arg:ident : $ty:ty ),* ) -> $ret:ty ; )*) => {
        /// Wszystkie komendy wystawiane przez `AppCore` (nazwa = komenda Tauri = metoda).
        pub const COMMANDS: &[&str] = &[$( stringify!($name) ),*];
    };
}

with_commands!(command_names);
