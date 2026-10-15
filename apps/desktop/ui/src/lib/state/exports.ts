// Eksport rozmowy do Markdown / HTML / JSON z menu wiadomości, menu sesji i palety poleceń: rdzeń składa
// plik (HTML renderowany w Rust, bez skryptów) i pyta o miejsce natywnym dialogiem.
import { agents as AGENTS } from '@alfa/ui-kit';
import type { ConversationFormat } from '../api/types-files';
import { conversationMarkdown } from '../logic/conversation-copy';
import type { AppState } from './app.svelte';

export async function exportConversation(
  app: AppState,
  sessionId: string | null,
  format: ConversationFormat,
  turnId: string | null = null,
): Promise<void> {
  if (!sessionId) return;
  try {
    const result = await app.client.conversation.exportConversation(sessionId, format, turnId);
    if (result.status === 'saved') {
      app.toasts.show({ kind: 'success', message: app.i18n.t('exp.saved', { path: result.path }) });
    }
  } catch (error) {
    app.toasts.show({
      kind: 'error',
      message: error instanceof Error ? error.message : String(error),
    });
  }
}

/** Bieżąca ścieżka rozmowy do schowka jako Markdown (paleta „Kopiuj rozmowę”). */
export async function copyConversation(app: AppState): Promise<void> {
  const conv = app.conversation;
  const { t } = app.i18n;
  if (!conv || conv.path.length === 0) {
    app.toasts.show({ kind: 'warning', message: t('copyConv.empty') });
    return;
  }
  const names = AGENTS as Record<string, { name: string } | undefined>;
  const text = conversationMarkdown(
    app.sessions.active?.title ?? t('copyConv.untitled'),
    conv.path,
    {
      you: t('copyConv.you'),
      agent: (id) => names[id]?.name ?? id,
      when: app.i18n.dateTime,
      aiNote: t('copyConv.aiNote'),
    },
  );
  try {
    await navigator.clipboard.writeText(text);
    app.toasts.show({ kind: 'success', message: t('copyConv.done') });
  } catch {
    app.toasts.show({ kind: 'error', message: t('msg.copyFailed') });
  }
}
