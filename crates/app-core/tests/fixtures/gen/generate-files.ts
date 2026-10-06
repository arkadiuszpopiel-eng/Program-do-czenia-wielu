// Fixture'y załączników composera (`attachments_*`, `turns_send` z `attachments`), eksportu rozmowy
// (`sessions_export_conversation`) i kopii zapasowych (`backups_*`) — część generatora `generate.ts`
// (atrapa + `TauriAlfaClient` z atrapą `invoke`). Wywoływana na końcu, żeby nie przesuwać
// identyfikatorów atrapy w pozostałych fixture'ach.
import type { VirtualScheduler } from '../../../../../apps/desktop/ui/src/lib/api/fake/fake-client';
import type {
  AttachmentInfo,
  AttachmentsAdded,
  BackupConfig,
  BackupView,
} from '../../../../../apps/desktop/ui/src/lib/api/types-files';

type Both = (ns: string, method: string, ...args: unknown[]) => Promise<unknown>;

export async function runFiles(
  both: Both,
  scheduler: VirtualScheduler,
  flush: () => Promise<void>,
): Promise<void> {
  await both('attachments', 'list', 's-trip');
  await both('attachments', 'pick', 's-trip');
  await both('attachments', 'paste', 's-trip');
  const dropped = (await both('attachments', 'addDropped', 's-trip', [
    { name: 'notatki.md', bytes: 400_000, mime: 'text/markdown' },
    { name: 'film.mp4', bytes: 30 * 1024 * 1024, mime: 'video/mp4' },
  ])) as AttachmentsAdded;
  const first = dropped.staged[0];
  if (!first) throw new Error('brak przygotowanego załącznika');
  await both('attachments', 'remove', 's-trip', first.id);
  const staged = (await both('attachments', 'list', 's-trip')) as AttachmentInfo[];
  await both('turns', 'send', 's-trip', {
    parent_id: null,
    text: 'Podsumuj załączniki',
    addressed_to: null,
    profile: 'local',
    attachments: staged.map((a) => a.id),
  });
  scheduler.runAll();
  await flush();
  await both('turns', 'list', 's-trip');
  await both('conversation', 'exportConversation', 's-trip', 'markdown', null);
  await both('conversation', 'exportConversation', 's-q3', 'html', 'q3');

  const start = (await both('backups', 'status')) as BackupView;
  const config: BackupConfig = { ...start.config, enabled: true, keep: 2, include_artifacts: true };
  await both('backups', 'configure', config);
  await both('backups', 'chooseDir');
  await both('backups', 'configure', config);
  await both('backups', 'setPassword', 'długie hasło kopii');
  await both('backups', 'runNow');
  const view = (await both('backups', 'runNow')) as BackupView;
  const newest = view.entries[0];
  if (!newest) throw new Error('brak kopii');
  await both('backups', 'verify', newest.file);
  await both('backups', 'setPassword', null);
}
