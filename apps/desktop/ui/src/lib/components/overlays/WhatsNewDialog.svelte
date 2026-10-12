<!--
  „Co nowego" (PLAN §14.8): raz po aktualizacji. Gdy dziennik zmian wbudowany w program zna tę
  wersję — lista wersji jak w Historii zmian (bieżąca rozwinięta, dziesięć ostatnich, starsze
  w zakładce, wagi kolorem i słowem). Inaczej notatki z podpisanej paczki jako zwykły tekst
  (bez HTML). Zamknięcie zapamiętuje wersję jako pokazaną.
-->
<script lang="ts">
  import { Dialog } from 'bits-ui';
  import { Button } from '@alfa/ui-kit';
  import type { ChangeLogEntry, WhatsNew } from '../../api/types-updates';
  import { entryFor } from '../../logic/change-history';
  import { useApp } from '../../state/context';
  import ChangeHistory from '../../views/settings/pages/ChangeHistory.svelte';

  interface Props {
    news: WhatsNew;
  }

  let { news }: Props = $props();
  const app = useApp();
  const { t } = app.i18n;
  /** Dziennik zmian; `null` — jeszcze się wczytuje albo się nie wczytał (zostają notatki). */
  let entries = $state<readonly ChangeLogEntry[] | null>(null);
  const current = $derived(entryFor(entries ?? [], news.version));

  $effect(() => {
    app.client.updates.history().then(
      (list) => (entries = list),
      () => (entries = null),
    );
  });

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
      {#if current && entries}
        <Dialog.Description class="alfa-news-lead">
          {current.summary || t('whatsNew.empty', { version: news.version })}
        </Dialog.Description>
        <!-- Lista przewija się sama; fokus trafia na jej przyciski (wersje, filtry, zakładki). -->
        <div class="alfa-news-list">
          <ChangeHistory embedded preloaded={entries} />
        </div>
      {:else}
        <Dialog.Description class="alfa-news-text">
          {news.notes.trim() || t('whatsNew.empty', { version: news.version })}
        </Dialog.Description>
      {/if}
      <div class="actions">
        <Button variant="secondary" onclick={settings}>{t('whatsNew.history')}</Button>
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
    width: min(680px, calc(var(--alfa-vw, 100vw) - 32px));
    max-height: min(calc(var(--alfa-vh, 100vh) * 0.85), 720px);
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
  :global(.alfa-news-lead) {
    margin: 0;
    color: var(--alfa-color-text-muted);
    font-size: var(--alfa-font-size-sm);
  }
  .alfa-news-list {
    flex: 1 1 auto;
    min-height: 0;
    overflow: auto;
    padding-right: var(--alfa-space-2);
  }
  .actions {
    display: flex;
    justify-content: flex-end;
    gap: var(--alfa-space-2);
  }
</style>
