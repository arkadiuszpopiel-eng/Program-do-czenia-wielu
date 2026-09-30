// Cienki adapter IPC Tauri 2: każda metoda = jedna komenda `invoke`, zdarzenia = jeden kanał `listen`.
// Nazwy komend i zdarzeń: COMMANDS.md (kontrakt dla sesji backendowej). Argumenty w camelCase —
// Tauri 2 mapuje je na parametry snake_case komend Rust.
import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import type { AlfaClient } from './client';
import type { AlfaEvent } from './types-system';

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
