// Załączniki composera aktywnej sesji: lista przygotowanych (kopie w katalogu sesji w rdzeniu),
// wybór / wklejenie / upuszczenie, odrzucenia jako toasty z powodem, komunikat dla czytnika ekranu.
import type { AttachmentInfo, AttachmentsAdded, DroppedFileHint } from '../api/types-files';
import type { AppState } from './app.svelte';

export class AttachmentsState {
  items = $state<readonly AttachmentInfo[]>([]);
  /** Pliki przeciągane nad oknem — podświetlenie strefy upuszczenia. */
  dragging = $state(false);
  /** Ostatni komunikat dla czytnika ekranu (`role="status"`). */
  status = $state('');
  private session: string | null = null;

  constructor(private readonly app: AppState) {}

  get ids(): string[] {
    return this.items.map((a) => a.id);
  }

  /** Lista przygotowanych w sesji (po przełączeniu sesji). */
  async load(sessionId: string | null): Promise<void> {
    this.session = sessionId;
    if (!sessionId) {
      this.items = [];
      return;
    }
    const list = await this.app.client.attachments.list(sessionId);
    if (this.session === sessionId) this.items = list;
  }

  private async target(): Promise<string | null> {
    if (!this.app.activeId) await this.app.newSession();
    return this.app.activeId;
  }

  private apply(sessionId: string, result: AttachmentsAdded): void {
    const { t } = this.app.i18n;
    if (this.session === sessionId || this.session === null) {
      this.session = sessionId;
      this.items = result.staged;
    }
    for (const r of result.rejected) {
      this.app.toasts.show({
        kind: 'warning',
        message: t('att.rejected', { name: r.name, reason: t(`att.reason.${r.reason}`) }),
      });
    }
    if (result.added.length > 0) this.status = t('att.added', { n: result.added.length });
  }

  private async run(action: (sessionId: string) => Promise<AttachmentsAdded>): Promise<void> {
    try {
      const sessionId = await this.target();
      if (!sessionId) return;
      this.apply(sessionId, await action(sessionId));
    } catch (error) {
      this.app.toasts.show({
        kind: 'error',
        message: error instanceof Error ? error.message : String(error),
      });
    }
  }

  pick(): Promise<void> {
    return this.run((id) => this.app.client.attachments.pick(id));
  }

  paste(): Promise<void> {
    return this.run((id) => this.app.client.attachments.paste(id));
  }

  drop(hints: readonly DroppedFileHint[]): Promise<void> {
    this.dragging = false;
    return this.run((id) => this.app.client.attachments.addDropped(id, hints));
  }

  async remove(attachment: AttachmentInfo): Promise<void> {
    const left = await this.app.client.attachments.remove(attachment.session_id, attachment.id);
    if (this.session === attachment.session_id) this.items = left;
    this.status = this.app.i18n.t('att.removed', { name: attachment.name });
  }

  /** Po wysłaniu tury: rdzeń zdjął wysłane z przygotowanych. */
  sent(ids: readonly string[]): void {
    this.items = this.items.filter((a) => !ids.includes(a.id));
  }
}
