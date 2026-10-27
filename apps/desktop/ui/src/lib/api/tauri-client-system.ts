// `AlfaClient['system']` w programie: komendy rdzenia i dziennik rozruchu (polecenie powłoki).
import { invoke } from '@tauri-apps/api/core';
import type { AlfaClient } from './client';
import { toError } from './command-error';
import { call } from './tauri-call';
import type { BootReport } from './types-boot';

export const sessionsTauri = (): AlfaClient['sessions'] => ({
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
  cards: (sessionIds) => call('sessions_cards', { sessionIds }),
  markRead: (sessionId) => call('sessions_mark_read', { sessionId }),
  getDraft: (sessionId) => call('sessions_get_draft', { sessionId }),
  saveDraft: (sessionId, text) => call('sessions_save_draft', { sessionId, text }),
  setProject: (sessionId, project) => call('sessions_set_project', { sessionId, project }),
  projectsList: () => call('projects_list'),
  projectsPickDir: () => call('projects_pick_dir'),
  projectsSave: (name, workdir) => call('projects_save', { name, workdir }),
  projectsRename: (oldName, newName) => call('projects_rename', { oldName, newName }),
  projectsRemove: (name) => call('projects_remove', { name }),
  pipelinesList: () => call('pipelines_list'),
  pipelinesSave: (pipeline, previousName) => call('pipelines_save', { pipeline, previousName }),
  pipelinesRemove: (name) => call('pipelines_remove', { name }),
  workdir: (sessionId) => call('sessions_workdir', { sessionId }),
  chooseWorkdir: (sessionId, choice) => call('sessions_choose_workdir', { sessionId, choice }),
});

export const systemTauri = (): AlfaClient['system'] => ({
  status: () => call('system_status'),
  retryQueue: () => call('system_retry_queue'),
  clipboardText: () => call('system_clipboard_text'),
  bootReport: () =>
    invoke<BootReport>('boot_report').catch((error: unknown) => {
      throw toError(error);
    }),
});
