// Eksport rozmowy do Markdown / HTML z menu wiadomości, menu sesji i palety poleceń: rdzeń składa
// plik (HTML renderowany w Rust, bez skryptów) i pyta o miejsce natywnym dialogiem.
import type { ConversationFormat } from '../api/types-files';
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
