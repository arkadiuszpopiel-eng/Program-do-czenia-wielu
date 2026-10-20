// Menu akcji rozmowy — jedno dla wiersza listy i karty panelu Sesje (te same pozycje i kolejność).
import type { MenuItem } from '@alfa/ui-kit';
import type { SessionSummary } from '../api/types';
import type { AppState } from './app.svelte';
import { exportConversation } from './exports';
import { duplicateSession } from './session-actions';
import { openStats } from './stats-dialog.svelte';

export function sessionMenuItems(app: AppState, session: SessionSummary): MenuItem[] {
  const { t } = app.i18n;
  return [
    {
      id: 'rename',
      label: t('sessions.rename'),
      onSelect: () => (app.sessions.renamingId = session.id),
    },
    {
      id: 'pin',
      label: t(session.pinned ? 'sessions.unpin' : 'sessions.pin'),
      onSelect: () => void app.setPinned(session.id, !session.pinned),
    },
    {
      id: 'archive',
      label: t(session.archived ? 'sessions.unarchive' : 'sessions.archive'),
      onSelect: () => void app.setArchived(session.id, !session.archived),
    },
    {
      id: 'duplicate',
      label: t('sessions.duplicate'),
      onSelect: () => void duplicateSession(app, session.id),
    },
    {
      id: 'export',
      label: t('sessions.export'),
      onSelect: () => void app.exportSession(session.id),
    },
    {
      id: 'export-md',
      label: t('exp.markdown'),
      onSelect: () => void exportConversation(app, session.id, 'markdown'),
    },
    {
      id: 'export-html',
      label: t('exp.html'),
      onSelect: () => void exportConversation(app, session.id, 'html'),
    },
    {
      id: 'export-json',
      label: t('exp.json'),
      onSelect: () => void exportConversation(app, session.id, 'json'),
    },
    {
      id: 'stats',
      label: t('stats.title'),
      // Statystyki liczą się z otwartej rozmowy — najpierw ją otwieramy.
      onSelect: () => void app.openSession(session.id).then(openStats),
    },
    {
      id: 'delete',
      label: t('sessions.delete'),
      danger: true,
      separatorBefore: true,
      onSelect: () => void app.deleteSession(session.id),
    },
  ];
}
