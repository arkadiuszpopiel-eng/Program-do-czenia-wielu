<!--
  „Tablica” (paleta `Ctrl+K`, P4): przypięte odpowiedzi agentek z różnych rozmów — treść (HTML
  zsanitowany w Rust), rozmowa, agentka i czas; „Otwórz rozmowę”, „Kopiuj”, „Odepnij”.
  Przypina się z menu wiadomości („Przypnij do tablicy”).
-->
<script lang="ts">
  import { Dialog } from 'bits-ui';
  import { Button, SanitizedHtml, agents as AGENTS } from '@alfa/ui-kit';
  import { errorText } from '../../api/command-error';
  import type { BoardItem } from '../../api/types-board';
  import { closeBoard } from '../../state/board-dialog.svelte';
  import { useApp } from '../../state/context';

  const app = useApp();
  const { t } = app.i18n;
  let items = $state<readonly BoardItem[] | null>(null);
  let error = $state<string | null>(null);

  $effect(() => {
    app.client.board.list().then(
      (list) => (items = list),
      (e: unknown) => (error = errorText(e)),
    );
  });

  function agentName(id: string): string {
    return (AGENTS as Record<string, { name: string } | undefined>)[id]?.name ?? id;
  }

  async function unpin(id: string) {
    try {
      items = await app.client.board.unpin(id);
    } catch (e) {
      app.toasts.show({ kind: 'error', message: errorText(e) });
    }
  }

  async function copy(text: string) {
    try {
      await navigator.clipboard.writeText(text);
      app.toasts.show({ kind: 'success', message: t('board.copied') });
    } catch (e) {
      app.toasts.show({ kind: 'error', message: errorText(e) });
    }
  }

  function open(item: BoardItem) {
    closeBoard();
    if (app.view === 'settings') app.closeSettings();
    void app.openSession(item.session_id);
  }
</script>

<Dialog.Root open onOpenChange={(open) => !open && closeBoard()}>
  <Dialog.Portal>
    <Dialog.Overlay class="alfa-board-overlay" />
    <Dialog.Content class="alfa-board">
      <Dialog.Title class="alfa-board-title">{t('board.title')}</Dialog.Title>
      <Dialog.Description class="alfa-board-lead">{t('board.lead')}</Dialog.Description>
      {#if error}<p role="alert">{error}</p>
      {:else if items && items.length === 0}
        <p class="empty">{t('board.empty')}</p>
      {:else if items}
        <ul class="list" aria-label={t('board.list')}>
          {#each items as item (item.id)}
            <li>
              <p class="meta">
                {agentName(item.agent)} · {item.session_title} · {app.i18n.dateTime(item.pinned_at)}
              </p>
              <div class="body"><SanitizedHtml html_sanitized={item.html_sanitized} /></div>
              <div class="actions">
                <Button size="sm" variant="secondary" onclick={() => open(item)}
                  >{t('board.open')}</Button
                >
                <Button size="sm" variant="secondary" onclick={() => void copy(item.text)}
                  >{t('board.copy')}</Button
                >
                <Button size="sm" variant="ghost" onclick={() => void unpin(item.id)}
                  >{t('board.unpin')}</Button
                >
              </div>
            </li>
          {/each}
        </ul>
      {/if}
      <div class="footer">
        <Button variant="primary" onclick={closeBoard}>{t('board.close')}</Button>
      </div>
    </Dialog.Content>
  </Dialog.Portal>
</Dialog.Root>

<style>
  :global(.alfa-board-overlay) {
    position: fixed;
    inset: 0;
    z-index: 110;
    background: var(--alfa-color-scrim);
  }
  :global(.alfa-board) {
    position: fixed;
    z-index: 111;
    top: 50%;
    left: 50%;
    display: flex;
    flex-direction: column;
    gap: var(--alfa-space-3);
    width: min(720px, calc(var(--alfa-vw, 100vw) - 32px));
    max-height: min(calc(var(--alfa-vh, 100vh) * 0.85), 760px);
    padding: var(--alfa-space-6);
    transform: translate(-50%, -50%);
    border: 1px solid var(--alfa-color-border);
    border-radius: var(--alfa-radius-overlay);
    background: var(--alfa-color-surface);
    box-shadow: var(--alfa-shadow-3);
    color: var(--alfa-color-text);
  }
  :global(.alfa-board-title) {
    margin: 0;
    font-size: var(--alfa-font-size-lg);
    font-weight: var(--alfa-weight-semibold);
  }
  :global(.alfa-board-lead),
  .meta,
  .empty {
    margin: 0;
    color: var(--alfa-color-text-muted);
    font-size: var(--alfa-font-size-sm);
  }
  .list {
    display: flex;
    flex: 1 1 auto;
    flex-direction: column;
    gap: var(--alfa-space-3);
    min-height: 0;
    margin: 0;
    padding: 0 var(--alfa-space-2) 0 0;
    overflow: auto;
    list-style: none;
  }
  .list > li {
    display: flex;
    flex-direction: column;
    gap: var(--alfa-space-2);
    padding: var(--alfa-space-3);
    border: 1px solid var(--alfa-color-border);
    border-radius: var(--alfa-radius-card);
  }
  .body {
    max-height: 220px;
    overflow: auto;
    font-size: var(--alfa-font-size-sm);
  }
  .actions,
  .footer {
    display: flex;
    flex-wrap: wrap;
    gap: var(--alfa-space-2);
  }
  .footer {
    justify-content: flex-end;
  }
</style>
