<!--
  „Co nowego" (PLAN §14.8): raz po aktualizacji — notatki z podpisanej paczki wersji, pokazane
  jako zwykły tekst (bez HTML). Zamknięcie zapamiętuje wersję jako pokazaną.
-->
<script lang="ts">
  import { Dialog } from 'bits-ui';
  import { Button } from '@alfa/ui-kit';
  import type { WhatsNew } from '../../api/types-updates';
  import { useApp } from '../../state/context';

  interface Props {
    news: WhatsNew;
  }

  let { news }: Props = $props();
  const app = useApp();
  const { t } = app.i18n;

  function close() {
    void app.updates.dismissWhatsNew(app.client.updates).catch(() => undefined);
  }

  function settings() {
    close();
    app.openSettings('updates');
  }
</script>

<Dialog.Root open onOpenChange={(open) => !open && close()}>
  <Dialog.Portal>
    <Dialog.Overlay class="alfa-news-overlay" />
    <Dialog.Content class="alfa-news">
      <Dialog.Title class="alfa-news-title">
        {t('whatsNew.title', { version: news.version })}
      </Dialog.Title>
      <Dialog.Description class="alfa-news-text">
        {news.notes.trim() || t('whatsNew.empty', { version: news.version })}
      </Dialog.Description>
      <div class="actions">
        <Button variant="secondary" onclick={settings}>{t('whatsNew.settings')}</Button>
        <Button variant="primary" onclick={close}>{t('whatsNew.close')}</Button>
      </div>
    </Dialog.Content>
  </Dialog.Portal>
</Dialog.Root>

<style>
  :global(.alfa-news-overlay) {
    position: fixed;
    inset: 0;
    z-index: 110;
    background: var(--alfa-color-scrim);
  }
  :global(.alfa-news) {
    position: fixed;
    z-index: 111;
    top: 50%;
    left: 50%;
    display: flex;
    flex-direction: column;
    gap: var(--alfa-space-3);
    width: min(560px, calc(100vw - 32px));
    max-height: min(80vh, 640px);
    padding: var(--alfa-space-6);
    transform: translate(-50%, -50%);
    border: 1px solid var(--alfa-color-border);
    border-radius: var(--alfa-radius-overlay);
    background: var(--alfa-color-surface);
    box-shadow: var(--alfa-shadow-3);
    color: var(--alfa-color-text);
  }
  :global(.alfa-news-title) {
    margin: 0;
    font-size: var(--alfa-font-size-lg);
    font-weight: var(--alfa-weight-semibold);
  }
  :global(.alfa-news-text) {
    margin: 0;
    overflow: auto;
    font-size: var(--alfa-font-size-sm);
    white-space: pre-wrap;
    overflow-wrap: anywhere;
  }
  .actions {
    display: flex;
    justify-content: flex-end;
    gap: var(--alfa-space-2);
  }
</style>
