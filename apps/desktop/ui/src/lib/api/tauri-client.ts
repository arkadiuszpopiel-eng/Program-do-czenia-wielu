// Cienki adapter IPC Tauri 2: każda metoda = jedna komenda `invoke`, zdarzenia = jeden kanał `listen`.
// Nazwy komend i zdarzeń: COMMANDS.md (kontrakt dla sesji backendowej). Argumenty w camelCase —
// Tauri 2 mapuje je na parametry snake_case komend Rust.
import { Channel, convertFileSrc, invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import type { AlfaClient } from './client';
import type { AlfaEvent } from './types-system';
import type { TerminalFrame } from './types-work';

/** Nazwa jedynego kanału zdarzeń rdzeń → UI (paczki zdarzeń, najwyżej jedna na klatkę). */
export const EVENTS_CHANNEL = 'alfa://events';

const call = <T>(command: string, args?: Record<string, unknown>): Promise<T> =>
  invoke<T>(command, args);

export class TauriAlfaClient implements AlfaClient {
  readonly kind = 'tauri' as const;
  private readonly unlisteners: UnlistenFn[] = [];

  readonly app: AlfaClient['app'] = {
    bootstrap: () => call('app_bootstrap'),
    completeOnboarding: () => call('app_complete_onboarding'),
    openSystemSettings: (uri) => call('app_open_system_settings', { uri }),
    saveLayout: (layout) => call('app_save_layout', { layout }),
    setActiveSession: (sessionId) => call('app_set_active_session', { sessionId }),
  };

  readonly sessions: AlfaClient['sessions'] = {
    list: () => call('sessions_list'),
    create: (template) => call('sessions_create', { template }),
    rename: (sessionId, title) => call('sessions_rename', { sessionId, title }),
    setPinned: (sessionId, pinned) => call('sessions_set_pinned', { sessionId, pinned }),
    setArchived: (sessionId, archived) => call('sessions_set_archived', { sessionId, archived }),
    remove: (sessionId) => call('sessions_remove', { sessionId }),
    undoRemove: (token) => call('sessions_undo_remove', { token }),
    duplicateAsTemplate: (sessionId) => call('sessions_duplicate_as_template', { sessionId }),
    exportSession: (sessionId) => call('sessions_export', { sessionId }),
    search: (query) => call('sessions_search', { query }),
    markRead: (sessionId) => call('sessions_mark_read', { sessionId }),
    getDraft: (sessionId) => call('sessions_get_draft', { sessionId }),
    saveDraft: (sessionId, text) => call('sessions_save_draft', { sessionId, text }),
    setProject: (sessionId, project) => call('sessions_set_project', { sessionId, project }),
    workdir: (sessionId) => call('sessions_workdir', { sessionId }),
    chooseWorkdir: (sessionId, choice) => call('sessions_choose_workdir', { sessionId, choice }),
  };

  readonly turns: AlfaClient['turns'] = {
    list: (sessionId) => call('turns_list', { sessionId }),
    send: (sessionId, options) => call('turns_send', { sessionId, options }),
    regenerate: (sessionId, turnId, profile) =>
      call('turns_regenerate', { sessionId, turnId, profile }),
    editAndResend: (sessionId, turnId, text) =>
      call('turns_edit_and_resend', { sessionId, turnId, text }),
    continueTurn: (sessionId, turnId) => call('turns_continue', { sessionId, turnId }),
    stop: (sessionId) => call('turns_stop', { sessionId }),
    rate: (turnId, rating) => call('turns_rate', { turnId, rating }),
    setHidden: (turnId, hidden) => call('turns_set_hidden', { turnId, hidden }),
    remember: (turnId, scope) => call('turns_remember', { turnId, scope }),
    readAloud: (turnId) => call('turns_read_aloud', { turnId }),
    saveCode: (turnId, blockIndex) => call('turns_save_code', { turnId, blockIndex }),
    runCode: (turnId, blockIndex) => call('turns_run_code', { turnId, blockIndex }),
    undoStep: (undoToken) => call('turns_undo_step', { undoToken }),
  };

  readonly agents: AlfaClient['agents'] = {
    list: (sessionId) => call('agents_list', { sessionId }),
    setRoles: (sessionId, agent, roleIds) =>
      call('agents_set_roles', { sessionId, agent, roleIds }),
    applyCast: (sessionId, template) => call('agents_apply_cast', { sessionId, template }),
    runs: (sessionId) => call('agents_runs', { sessionId }),
    steer: (sessionId, text) => call('agents_steer', { sessionId, text }),
    openTerminal: (stepId) => call('agents_open_terminal', { stepId }),
  };

  readonly costs: AlfaClient['costs'] = {
    summary: (sessionId) => call('costs_summary', { sessionId }),
    setMonthlyLimit: (enabled, monthly) => call('costs_set_monthly_limit', { enabled, monthly }),
  };

  readonly settings: AlfaClient['settings'] = {
    schema: () => call('settings_schema'),
    values: () => call('settings_values'),
    set: (key, value) => call('settings_set', { key, value }),
    reset: (key) => call('settings_reset', { key }),
    setShortcut: (actionId, chord) => call('settings_set_shortcut', { actionId, chord }),
  };

  readonly timeline: AlfaClient['timeline'] = {
    list: (sessionId, filter) => call('timeline_list', { sessionId, filter }),
  };

  readonly files: AlfaClient['files'] = {
    list: (sessionId) => call('files_list', { sessionId }),
    preview: (artifactId) => call('files_preview', { artifactId }),
    act: (artifactId, action) => call('files_act', { artifactId, action }),
  };

  readonly accounts: AlfaClient['accounts'] = {
    catalog: () => call('accounts_catalog'),
    list: () => call('accounts_list'),
    add: (input) => call('accounts_add', { input }),
    test: (accountId) => call('accounts_test', { accountId }),
    assign: (accountId, assignment) => call('accounts_assign', { accountId, assignment }),
    setLimit: (accountId, enabled, monthly) =>
      call('accounts_set_limit', { accountId, enabled, monthly }),
    remove: (accountId) => call('accounts_remove', { accountId }),
  };

  readonly transfer: AlfaClient['transfer'] = {
    exportPackage: (request) => call('transfer_export', { request }),
    inspect: (password, path) => call('transfer_inspect', { password, path }),
    importPackage: (request) => call('transfer_import', { request }),
    rollback: (snapshotId) => call('transfer_rollback', { snapshotId }),
  };

  readonly permissions: AlfaClient['permissions'] = {
    get: (sessionId) => call('permissions_get', { sessionId }),
    requestLevel: (level, sessionId) => call('permissions_request_level', { level, sessionId }),
    openApproval: (approvalId) => call('permissions_open_approval', { approvalId }),
  };

  readonly models: AlfaClient['models'] = {
    localList: () => call('models_local_list'),
    localDownload: (modelId) => call('models_local_download', { modelId }),
    localCancel: (modelId) => call('models_local_cancel', { modelId }),
  };

  readonly device: AlfaClient['device'] = {
    profile: () => call('device_profile'),
    measure: () => call('device_measure'),
  };

  readonly voice: AlfaClient['voice'] = {
    devices: () => call('voice_devices'),
    startMicTest: (deviceId) => call('voice_start_mic_test', { deviceId }),
    stopMicTest: () => call('voice_stop_mic_test'),
    setMicEnabled: (enabled) => call('voice_set_mic_enabled', { enabled }),
    setMuted: (muted) => call('voice_set_muted', { muted }),
    stopSpeech: () => call('voice_stop_speech'),
    status: () => call('voice_status'),
    ptt: (pressed) => call('voice_ptt', { pressed }),
    preview: (agent) => call('voice_preview', { agent }),
  };

  readonly system: AlfaClient['system'] = {
    status: () => call('system_status'),
    retryQueue: () => call('system_retry_queue'),
  };

  readonly quick: AlfaClient['quick'] = {
    ask: (text) => call('quick_ask', { text }),
    expandToMain: (sessionId) => call('quick_expand_to_main', { sessionId }),
    hide: () => call('quick_hide'),
  };

  readonly memory: AlfaClient['memory'] = {
    status: () => call('memory_status'),
    scopes: () => call('memory_scopes'),
    inspect: (query) => call('memory_inspect', { query }),
    explain: (entryId) => call('memory_explain', { entryId }),
    edit: (entryId, edit) => call('memory_edit', { entryId, edit }),
    setPinned: (entryId, pinned) => call('memory_set_pinned', { entryId, pinned }),
    approve: (entryId) => call('memory_approve', { entryId }),
    promote: (entryId, to) => call('memory_promote', { entryId, to }),
    forgetPreview: (target) => call('memory_forget_preview', { target }),
    forget: (target) => call('memory_forget', { target }),
    journal: (scope) => call('memory_journal', { scope }),
    undo: (scope, changeId) => call('memory_undo', { scope, changeId }),
    consolidateNow: () => call('memory_consolidate_now'),
  };

  readonly tasks: AlfaClient['tasks'] = {
    list: () => call('tasks_list'),
    create: (input) => call('tasks_create', { input }),
    cancel: (taskId) => call('tasks_cancel', { taskId }),
    retry: (taskId) => call('tasks_retry', { taskId }),
    steer: (taskId, text) => call('tasks_steer', { taskId, text }),
    pause: (taskId) => call('tasks_pause', { taskId }),
    resume: (taskId) => call('tasks_resume', { taskId }),
  };

  readonly triggers: AlfaClient['triggers'] = {
    list: () => call('triggers_list'),
    create: (draft) => call('triggers_create', { draft }),
    remove: (triggerId) => call('triggers_remove', { triggerId }),
    setEnabled: (triggerId, enabled) => call('triggers_set_enabled', { triggerId, enabled }),
    fireNow: (triggerId) => call('triggers_fire_now', { triggerId }),
    log: (triggerId) => call('triggers_log', { triggerId }),
    previewCron: (expr) => call('triggers_preview_cron', { expr }),
  };

  readonly marshal: AlfaClient['marshal'] = {
    state: () => call('marshal_state'),
    propose: (text, drafts) => call('marshal_propose', { text, drafts }),
    approve: (proposalId) => call('marshal_approve', { proposalId }),
    reject: (proposalId) => call('marshal_reject', { proposalId }),
    revoke: (ruleId) => call('marshal_revoke', { ruleId }),
    report: () => call('marshal_report'),
  };

  readonly bridges: AlfaClient['bridges'] = {
    list: (refresh) => call('bridges_list', { refresh }),
    setEnabled: (routeId, enabled) => call('bridges_set_enabled', { routeId, enabled }),
    setSchedule: (bridge, perDay) => call('bridges_set_schedule', { bridge, perDay }),
    pin: (bridge, version) => call('bridges_pin', { bridge, version }),
    openLogin: (bridge) => call('bridges_open_login', { bridge }),
  };

  readonly gui: AlfaClient['gui'] = {
    status: () => call('gui_status'),
    screenshot: () => call('gui_screenshot'),
    stop: () => call('gui_stop'),
    release: () => call('gui_release'),
    desktopGrant: (sessionId, agent) => call('gui_desktop_grant', { sessionId, agent }),
  };

  readonly terminal: AlfaClient['terminal'] = {
    // Strumień VT wyłącznie kanałem IPC tego wywołania (nie `alfa://events`).
    open: (profile, cols, rows, cwd, onFrame) => {
      const channel = new Channel<TerminalFrame>();
      channel.onmessage = onFrame;
      return call('terminal_open', { profile, cols, rows, cwd, channel });
    },
    input: (terminal, dataB64) => call('terminal_input', { terminal, dataB64 }),
    resize: (terminal, cols, rows) => call('terminal_resize', { terminal, cols, rows }),
    close: (terminal) => call('terminal_close', { terminal }),
    list: () => call('terminal_list'),
  };

  readonly skills: AlfaClient['skills'] = {
    list: () => call('skills_list'),
    review: (skillId, version) => call('skills_review', { skillId, version }),
    propose: (skill) => call('skills_propose', { skill }),
    approve: (skillId, version, hash) => call('skills_approve', { skillId, version, hash }),
    release: (skillId, version, hash) => call('skills_release', { skillId, version, hash }),
    reject: (skillId, version) => call('skills_reject', { skillId, version }),
    disable: (skillId) => call('skills_disable', { skillId }),
    run: (skillId, sessionId, agent, params) =>
      call('skills_run', { skillId, sessionId, agent, params }),
    exportBundle: () => call('skills_export'),
    importBundle: () => call('skills_import'),
  };

  readonly builder: AlfaClient['builder'] = {
    policy: () => call('builder_policy'),
    propose: (description) => call('builder_propose', { description }),
    preview: (draft) => call('builder_preview', { draft }),
    dryRun: (draft) => call('builder_dry_run', { draft }),
    save: (draft, hash) => call('builder_save', { draft, hash }),
    voicePreview: (draft) => call('builder_voice_preview', { draft }),
    library: () => call('builder_library'),
  };

  readonly health: AlfaClient['health'] = {
    report: () => call('health_report'),
    scan: () => call('health_scan'),
    approve: (repairId) => call('health_approve', { repairId }),
    reject: (repairId) => call('health_reject', { repairId }),
    undo: (repairId) => call('health_undo', { repairId }),
    improver: () => call('improver_list'),
    improverCycle: () => call('improver_cycle'),
    improverApprove: (proposalId, digest) => call('improver_approve', { proposalId, digest }),
    improverReject: (proposalId) => call('improver_reject', { proposalId }),
    improverRollback: (proposalId) => call('improver_rollback', { proposalId }),
    evals: () => call('evals_list'),
    evalsVerify: (suiteId) => call('evals_verify', { suiteId }),
  };

  readonly updates: AlfaClient['updates'] = {
    status: () => call('updates_status'),
    check: () => call('updates_check'),
    download: () => call('updates_download'),
    cancel: () => call('updates_cancel'),
    restart: () => call('updates_restart'),
    rollback: () => call('updates_rollback'),
    about: () => call('updates_about'),
    whatsNew: () => call('updates_whats_new'),
    dismissWhatsNew: () => call('updates_dismiss_whats_new'),
  };

  readonly broker: AlfaClient['broker'] = {
    status: () => call('broker_status'),
  };

  readonly voiceFeatures: AlfaClient['voiceFeatures'] = {
    features: () => call('voice_features'),
    wake: (action) => call('voice_wake', { action }),
    speaker: (action) => call('voice_speaker', { action }),
    dictation: (action) => call('voice_dictation', { action }),
    read: (action) => call('voice_read', { action }),
  };

  readonly plugins: AlfaClient['plugins'] = {
    list: () => call('plugins_list'),
    inspect: (wasmB64) => call('plugins_inspect', { wasmB64 }),
    propose: (manifest, wasmB64) => call('plugins_propose', { manifest, wasmB64 }),
    approve: (pluginId, version, reviewedHash) =>
      call('plugins_approve', { pluginId, version, reviewedHash }),
    reject: (pluginId, version) => call('plugins_reject', { pluginId, version }),
    disable: (pluginId) => call('plugins_disable', { pluginId }),
    enable: (pluginId, reviewedHash) => call('plugins_enable', { pluginId, reviewedHash }),
    remove: (pluginId) => call('plugins_remove', { pluginId }),
  };

  readonly engines: AlfaClient['engines'] = {
    list: () => call('models_list'),
    download: (itemId) => call('models_download', { itemId }),
    cancel: (itemId) => call('models_cancel', { itemId }),
    verify: (itemId) => call('models_verify', { itemId }),
    remove: (itemId) => call('models_remove', { itemId }),
    trustHash: (itemId, hashes) => call('models_trust_hash', { itemId, hashes }),
    activateEmbedder: (model) => call('embed_model_activate', { model }),
    reindexStart: () => call('search_reindex_start'),
    reindexCancel: () => call('search_reindex_cancel'),
    reindexStatus: () => call('search_reindex_status'),
  };

  readonly attachments: AlfaClient['attachments'] = {
    pick: (sessionId) => call('attachments_pick', { sessionId }),
    // Ścieżki upuszczenia zna wyłącznie powłoka (zdarzenie systemowe) — wskazówek z UI nie wysyłamy.
    addDropped: (sessionId) => call('attachments_add_dropped', { sessionId }),
    paste: (sessionId) => call('attachments_paste', { sessionId }),
    list: (sessionId) => call('attachments_list', { sessionId }),
    remove: (sessionId, attachmentId) => call('attachments_remove', { sessionId, attachmentId }),
    watchDrag: (handler) => {
      let active = true;
      const offs: UnlistenFn[] = [];
      const phases = [
        ['tauri://drag-enter', 'enter'],
        ['tauri://drag-leave', 'leave'],
        ['tauri://drag-drop', 'drop'],
      ] as const;
      for (const [event, phase] of phases) {
        void listen(event, () => {
          if (active) handler(phase);
        }).then((off) => (active ? offs.push(off) : off()));
      }
      return () => {
        active = false;
        for (const off of offs.splice(0)) off();
      };
    },
    previewUrl: (path) => convertFileSrc(path),
  };

  readonly conversation: AlfaClient['conversation'] = {
    exportConversation: (sessionId, format, turnId) =>
      call('sessions_export_conversation', { sessionId, format, turnId }),
  };

  readonly backups: AlfaClient['backups'] = {
    status: () => call('backups_status'),
    configure: (config) => call('backups_configure', { config }),
    chooseDir: () => call('backups_choose_dir'),
    setPassword: (password) => call('backups_set_password', { password }),
    runNow: () => call('backups_run_now'),
    verify: (file) => call('backups_verify', { file }),
  };

  subscribe(handler: (batch: readonly AlfaEvent[]) => void): () => void {
    let active = true;
    let unlisten: UnlistenFn | null = null;
    void listen<readonly AlfaEvent[]>(EVENTS_CHANNEL, (event) => {
      if (active) handler(event.payload);
    }).then((fn) => {
      if (active) {
        unlisten = fn;
        this.unlisteners.push(fn);
      } else fn();
    });
    return () => {
      active = false;
      unlisten?.();
    };
  }

  dispose(): void {
    for (const fn of this.unlisteners.splice(0)) fn();
  }
}
