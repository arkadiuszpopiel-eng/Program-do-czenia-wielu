// Atrapa: import/eksport `.alfa` (wydzielone z `api-hub.ts` — limit 400 linii na plik).
import type { AlfaClient } from '../client';
import type { DryRunItem } from '../types-hub';
import type { FakeCore } from './core';
import {
  HANDLE_EXPIRED,
  REVIEW_HANDLE_TTL_MS,
  issueHandle,
  takeHandle,
  type HandleTarget,
} from './transfer-handles';

const DRY_RUN: readonly DryRunItem[] = [
  { key: 'config.common', kind: 'config', label: 'Konfiguracja wspólna', diff: 'changed' },
  { key: 'personas', kind: 'persona', label: 'Agentki i biblie głosów', diff: 'same' },
  { key: 'casts', kind: 'cast', label: 'Obsady ról (4)', diff: 'new' },
  { key: 'session:s-q3', kind: 'session', label: 'Raport Q3', diff: 'collision' },
  { key: 'session:s-laptop-1', kind: 'session', label: 'Notatki z laptopa', diff: 'new' },
];

export function transferApi(core: FakeCore): AlfaClient['transfer'] {
  return {
    exportPackage: (request) => {
      const files = 3 + request.scope.sessions.length * 3;
      return core.reply({
        status: 'saved' as const,
        path: 'C:\\Users\\Ty\\Documents\\alfa-eksport-2026-09-30.alfa',
        files,
        bytes: 12_000 + files * 4_096,
      });
    },
    inspect: (password, handle) => {
      // Bez uchwytu — „natywny dialog” (atrapa: paczka z laptopa); inaczej uchwyt zużywany.
      const dialog: HandleTarget = {
        path: 'C:\\Users\\Ty\\Pobrane\\laptop.alfa',
        encrypted: false,
      };
      const target = handle === null ? dialog : takeHandle(core, handle);
      if (!target) return Promise.reject(new Error(HANDLE_EXPIRED));
      const next = issueHandle(core, target, REVIEW_HANDLE_TTL_MS);
      if (target.encrypted && !password) {
        return core.reply({ status: 'needs_password' as const, handle: next });
      }
      return core.reply({
        status: 'inspected' as const,
        handle: next,
        manifest: {
          schema_version: '1.0.0',
          app_version: '0.1.0-f1',
          created_at: new Date(core.scheduler.now() - 86_400_000).toISOString(),
          source_machine: 'LAPTOP-ALFA (laptop-cuda)',
          encrypted: Boolean(password),
        },
        items: DRY_RUN,
        warnings: ['Nakładka maszyny z laptopa nie zostanie zaimportowana.'],
        migrations: [],
      });
    },
    importPackage: (request) => {
      if (!takeHandle(core, request.handle)) return Promise.reject(new Error(HANDLE_EXPIRED));
      return core.reply({
        snapshot_id: core.nextId('snap'),
        imported: DRY_RUN.filter((i) => i.diff !== 'same').length,
        skipped: request.mode === 'add' ? 1 : 0,
        failed: 0,
      });
    },
    rollback: () => core.reply(undefined),
  };
}
