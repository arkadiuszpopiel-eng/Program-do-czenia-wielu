//! Komendy computer use (`gui_*`), terminala (`terminal_*`), umiejętności (`skills_*`), Kreatora
//! (`builder_*`), „Zdrowia systemu" (`health_*`, `improver_*`, `evals_*`), aktualizacji
//! (`updates_*`), wtyczek (`plugins_*`) i modeli (`models_*`, `embed_model_activate`,
//! `search_reindex_*`) — delegują do `app-gui`, `app-terminal`, `app-skills`, `app-health`,
//! `app-updates`, `app-plugins`, `app-models`, `app-files` (działania jako użytkownik w UI).

use std::sync::Arc;

use crate::core::AppCore;
use crate::dto::{
    AboutInfo, AgentDraft, AttachmentInfo, AttachmentsAdded, BackupCheck, BackupConfig, BackupView,
    BrokerIntentResult, BuilderAgentInfo, BuilderDryRun, BuilderPolicyView, BuilderPreview,
    BuilderProposal, BuilderSaved, ConversationFormat, EmbedderView, EvalSuiteView, EvalsView,
    ExportResult, GuiScreenshot, GuiStatus, HealthView, ImproverView, ModelItem, ModelsView,
    PluginInfo, PluginInspection, PluginsView, ReindexView, SecretInput, SkillImportResult,
    SkillInfo, SkillReview, TaskInfo, TerminalProfileId, TerminalSession, TrustedHashes,
    UpdatesView, WhatsNew,
};
use crate::error::AppError;

/// `ok` — wynik bez błędu, `res` — `Result`, `wait` — `async` z `Result`.
macro_rules! work_commands {
    ($( $kind:ident $name:ident ( $( $arg:ident : $ty:ty ),* ) -> $ret:ty = $part:ident . $method:ident ( $( $call:expr ),* ) ; )*) => {
        impl AppCore {
            $(
                #[doc = concat!("`", stringify!($name), "` (COMMANDS.md) — `", stringify!($part), "::", stringify!($method), "`.")]
                pub async fn $name(&self, $( $arg: $ty ),*) -> Result<$ret, AppError> {
                    work_commands!(@call $kind self.inner.work.$part.$method($( $call ),*))
                }
            )*
        }
    };
    (@call ok $e:expr) => { Ok($e) };
    (@call res $e:expr) => { $e };
    (@call wait $e:expr) => { $e.await };
}

work_commands! {
    ok gui_status() -> GuiStatus = gui.status();
    ok gui_screenshot() -> Option<GuiScreenshot> = gui.screenshot();
    ok gui_release() -> GuiStatus = gui.release();
    res terminal_input(terminal: u64, data_b64: String) -> () = terminal.input(terminal, &data_b64);
    res terminal_resize(terminal: u64, cols: u16, rows: u16) -> () = terminal.resize(terminal, cols, rows);
    res terminal_close(terminal: u64) -> () = terminal.close(terminal);
    ok terminal_list() -> Vec<TerminalSession> = terminal.list();
    res skills_list() -> Vec<SkillInfo> = skills.list();
    res skills_review(skill_id: String, version: String) -> SkillReview = skills.review(&skill_id, &version);
    wait skills_propose(skill: serde_json::Value) -> SkillInfo = skills.propose(skill);
    wait skills_approve(skill_id: String, version: String, hash: String) -> SkillInfo = skills.approve(&skill_id, &version, &hash);
    wait skills_release(skill_id: String, version: String, hash: String) -> SkillInfo = skills.release(&skill_id, &version, &hash);
    wait skills_reject(skill_id: String, version: String) -> SkillInfo = skills.reject(&skill_id, &version);
    wait skills_disable(skill_id: String) -> SkillInfo = skills.disable(&skill_id);
    wait skills_export() -> ExportResult = skills.export();
    wait skills_import() -> SkillImportResult = skills.import();
    res builder_policy() -> BuilderPolicyView = builder.policy();
    res builder_propose(description: String) -> BuilderProposal = builder.propose(&description);
    res builder_preview(draft: AgentDraft) -> BuilderPreview = builder.preview(&draft);
    wait builder_dry_run(draft: AgentDraft) -> BuilderDryRun = builder.dry_run(&draft);
    wait builder_save(draft: AgentDraft, hash: String) -> BuilderSaved = builder.save(&draft, &hash);
    wait builder_voice_preview(draft: AgentDraft) -> () = builder.voice_preview(&draft);
    res builder_library() -> Vec<BuilderAgentInfo> = builder.library();
    wait health_report() -> HealthView = health.report();
    wait health_scan() -> HealthView = health.scan();
    wait health_approve(repair_id: u64) -> HealthView = health.approve(repair_id);
    wait health_reject(repair_id: u64) -> HealthView = health.reject(repair_id);
    wait health_undo(repair_id: u64) -> HealthView = health.undo(repair_id);
    res improver_list() -> ImproverView = health.improver_view();
    wait improver_cycle() -> ImproverView = health.cycle();
    wait improver_approve(proposal_id: u64, digest: String) -> ImproverView = health.improver_approve(proposal_id, &digest);
    wait improver_reject(proposal_id: u64) -> ImproverView = health.improver_reject(proposal_id);
    wait improver_rollback(proposal_id: u64) -> ImproverView = health.improver_rollback(proposal_id);
    ok evals_list() -> EvalsView = health.evals_view();
    res evals_verify(suite_id: String) -> EvalSuiteView = health.verify(&suite_id);
    wait updates_status() -> UpdatesView = updates.status();
    wait updates_check() -> UpdatesView = updates.check();
    wait updates_download() -> UpdatesView = updates.download();
    wait updates_cancel() -> UpdatesView = updates.cancel();
    wait updates_restart() -> () = updates.restart();
    wait updates_rollback() -> UpdatesView = updates.rollback();
    wait updates_about() -> AboutInfo = updates.about();
    wait updates_whats_new() -> Option<WhatsNew> = updates.whats_new();
    wait updates_dismiss_whats_new() -> () = updates.dismiss_whats_new();
    res plugins_list() -> PluginsView = plugins.list();
    wait plugins_inspect(wasm_b64: String) -> PluginInspection = plugins.inspect(&wasm_b64);
    wait plugins_propose(manifest: serde_json::Value, wasm_b64: String) -> PluginInfo = plugins.propose(manifest, &wasm_b64);
    wait plugins_approve(plugin_id: String, version: String, reviewed_hash: String) -> PluginInfo = plugins.approve(&plugin_id, &version, &reviewed_hash);
    wait plugins_reject(plugin_id: String, version: String) -> PluginInfo = plugins.reject(&plugin_id, &version);
    wait plugins_disable(plugin_id: String) -> PluginInfo = plugins.disable(&plugin_id);
    wait plugins_enable(plugin_id: String, reviewed_hash: String) -> PluginInfo = plugins.enable(&plugin_id, &reviewed_hash);
    wait plugins_remove(plugin_id: String) -> PluginsView = plugins.remove(&plugin_id);
    wait models_list() -> ModelsView = models.list();
    wait models_download(item_id: String) -> ModelItem = models.download(&item_id);
    wait models_cancel(item_id: String) -> ModelItem = models.cancel(&item_id);
    wait models_verify(item_id: String) -> ModelItem = models.verify(&item_id);
    wait models_remove(item_id: String) -> ModelItem = models.remove(&item_id);
    wait models_trust_hash(item_id: String, hashes: TrustedHashes) -> ModelItem = models.trust_hash(&item_id, hashes);
    wait models_repair(item_id: String) -> ModelItem = models.repair(&item_id);
    wait embed_model_activate(model: String) -> EmbedderView = models.activate_embedder(&model);
    wait search_reindex_start() -> ReindexView = models.reindex_start();
    wait search_reindex_cancel() -> ReindexView = models.reindex_cancel();
    wait search_reindex_status() -> ReindexView = models.reindex_status();
    wait attachments_pick(session_id: String) -> AttachmentsAdded = files.attachments_pick(&session_id);
    wait attachments_add_dropped(session_id: String) -> AttachmentsAdded = files.attachments_add_dropped(&session_id);
    wait attachments_paste(session_id: String) -> AttachmentsAdded = files.attachments_paste(&session_id);
    res attachments_list(session_id: String) -> Vec<AttachmentInfo> = files.attachments_list(&session_id);
    res attachments_remove(session_id: String, attachment_id: String) -> Vec<AttachmentInfo> = files.attachments_remove(&session_id, &attachment_id);
    wait sessions_export_conversation(session_id: String, format: ConversationFormat, turn_id: Option<String>) -> ExportResult = files.export_conversation(&session_id, format, turn_id);
    ok backups_status() -> BackupView = files.backups_status();
    res backups_configure(config: BackupConfig) -> BackupView = files.backups_configure(config);
    wait backups_choose_dir() -> BackupView = files.backups_choose_dir();
    res backups_set_password(password: Option<SecretInput>) -> BackupView = files.backups_set_password(password);
    wait backups_run_now() -> BackupView = files.backups_run_now();
    wait backups_verify(file: String) -> BackupCheck = files.backups_verify(&file);
}

impl AppCore {
    /// Powłoka: pliki upuszczone na okno główne (ścieżki z systemu — pobiera je
    /// `attachments_add_dropped`; UI nie podaje ścieżek).
    pub fn attachments_dropped(&self, paths: Vec<std::path::PathBuf>) {
        self.inner.work.files.dropped(paths);
    }

    /// `gui_stop`: „Zatrzymaj sterowanie" — anuluje trwające akcje GUI i przebiegi sterujących
    /// agentek, narzędzia GUI wstrzymane do `gui_release` (właściciel przejmuje mysz i klawiaturę).
    pub async fn gui_stop(&self) -> Result<GuiStatus, AppError> {
        for session in self.inner.work.gui.take_over() {
            let _ = self.turns_stop(session.to_string()).await;
        }
        Ok(self.inner.work.gui.status())
    }

    /// `gui_desktop_grant`: prośba do Brokera o „zawsze zezwalaj na podgląd pulpitu" agentce
    /// w sesji (zakres ≤ 24 h wybiera właściciel w oknie Brokera).
    pub async fn gui_desktop_grant(
        &self,
        session_id: String,
        agent: String,
    ) -> Result<BrokerIntentResult, AppError> {
        let session = crate::ids::session(&session_id)?;
        self.ensure_session(&session)?;
        self.inner.work.gui.desktop_grant(session, &agent).await
    }

    /// `terminal_open` (komenda ze strumieniem): wyłącznie z gestu użytkownika w panelu terminala;
    /// wyjście VT trafia tylko do `sink` (kanał `tauri::ipc::Channel` powłoki).
    pub async fn terminal_open(
        &self,
        profile: TerminalProfileId,
        cols: u16,
        rows: u16,
        cwd: Option<String>,
        sink: Arc<dyn app_terminal::FrameSink>,
    ) -> Result<TerminalSession, AppError> {
        self.inner
            .work
            .terminal
            .open(profile, cols, rows, cwd, sink)
    }

    /// `skills_run`: umiejętność jako zadanie agentki w sesji (koperta ≤ jej roli).
    pub async fn skills_run(
        &self,
        skill_id: String,
        session_id: String,
        agent: Option<String>,
        params: serde_json::Value,
    ) -> Result<TaskInfo, AppError> {
        self.ensure_session(&crate::ids::session(&session_id)?)?;
        self.inner
            .work
            .skills
            .run(&skill_id, &session_id, agent.as_deref(), params)
    }
}
