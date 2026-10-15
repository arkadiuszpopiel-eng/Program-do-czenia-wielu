// Konta i klucze API, import/eksport `.alfa` i silniki modeli w kliencie Tauri — wydzielone
// z `tauri-client.ts` (limit 400 linii na plik).
import type { AlfaClient } from './client';
import { call } from './tauri-call';

export const accountsApi = (): AlfaClient['accounts'] => ({
  catalog: () => call('accounts_catalog'),
  list: () => call('accounts_list'),
  add: (input) => call('accounts_add', { input }),
  test: (accountId) => call('accounts_test', { accountId }),
  assign: (accountId, assignment) => call('accounts_assign', { accountId, assignment }),
  setLimit: (accountId, enabled, monthly) =>
    call('accounts_set_limit', { accountId, enabled, monthly }),
  remove: (accountId) => call('accounts_remove', { accountId }),
  rotate: (accountId, secret) => call('accounts_rotate', { accountId, secret }),
  setDisabled: (accountId, disabled) => call('accounts_set_disabled', { accountId, disabled }),
  setModel: (accountId, model) => call('accounts_set_model', { accountId, model }),
  setQueue: (accountId, taskClass, models) =>
    call('accounts_set_queue', { accountId, taskClass, models }),
  suggestQueue: (accountId, taskClass, strategy) =>
    call('accounts_suggest_queue', { accountId, taskClass, strategy }),
  sharedQueues: () => call('accounts_shared_queues'),
  setSharedQueue: (taskClass, entries) => call('accounts_set_shared_queue', { taskClass, entries }),
  routing: () => call('routing_get'),
  setRouting: (settings) => call('routing_set', { settings }),
  routingPreview: (text, taskClass) => call('routing_preview', { text, taskClass }),
  importEnv: () => call('accounts_import_env'),
});

export const transferApi = (): AlfaClient['transfer'] => ({
  exportPackage: (request) => call('transfer_export', { request }),
  inspect: (password, handle) => call('transfer_inspect', { password, handle }),
  importPackage: (request) => call('transfer_import', { request }),
  rollback: (snapshotId) => call('transfer_rollback', { snapshotId }),
});

export const enginesApi = (): AlfaClient['engines'] => ({
  list: () => call('models_list'),
  download: (itemId) => call('models_download', { itemId }),
  cancel: (itemId) => call('models_cancel', { itemId }),
  verify: (itemId) => call('models_verify', { itemId }),
  remove: (itemId) => call('models_remove', { itemId }),
  trustHash: (itemId, hashes) => call('models_trust_hash', { itemId, hashes }),
  repair: (itemId) => call('models_repair', { itemId }),
  bundles: () => call('models_bundles'),
  bundleDownload: (bundleId) => call('models_bundle_download', { bundleId }),
  bundleVerify: (bundleId) => call('models_bundle_verify', { bundleId }),
  activateEmbedder: (model) => call('embed_model_activate', { model }),
  reindexStart: () => call('search_reindex_start'),
  reindexCancel: () => call('search_reindex_cancel'),
  reindexStatus: () => call('search_reindex_status'),
});

export const mcpApi = (): AlfaClient['mcp'] => ({
  list: () => call('mcp_list'),
  save: (input) => call('mcp_save', { input }),
  remove: (id) => call('mcp_remove', { id }),
  tools: (id) => call('mcp_tools', { id }),
  approve: (id, tool, fingerprint) => call('mcp_approve', { id, tool, fingerprint }),
  revoke: (id, tool) => call('mcp_revoke', { id, tool }),
});

export const diagApi = (): AlfaClient['diag'] => ({
  state: () => call('diag_state'),
  snapshot: () => call('diag_snapshot'),
  run: (tests) => call('diag_run', { tests }),
  cancel: () => call('diag_cancel'),
  runs: (limit) => call('diag_runs', { limit }),
  export: () => call('diag_export'),
});
