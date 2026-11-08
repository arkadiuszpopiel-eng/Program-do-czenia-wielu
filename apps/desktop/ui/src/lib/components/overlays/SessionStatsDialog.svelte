<!--
  „Statystyki rozmowy” (paleta `Ctrl+K`, P5): wiadomości, słowa, czas czytania, tokeny, koszt,
  średni czas odpowiedzi, narzędzia, okres, inne gałęzie i modele bieżącej ścieżki. Liczone w UI
  (`logic/session-stats`); „Kopiuj jako Markdown” do schowka.
-->
<script lang="ts">
  import { Dialog } from 'bits-ui';
  import { Button } from '@alfa/ui-kit';
  import { sessionStats, statsMarkdown } from '../../logic/session-stats';
  import { useApp } from '../../state/context';
  import { closeStats } from '../../state/stats-dialog.svelte';

  const app = useApp();
  const { t } = app.i18n;
  const conversation = $derived(app.conversation);
  const stats = $derived(
    conversation
      ? sessionStats(conversation.path, Object.keys(conversation.tree.turns).length)
      : null,
  );
  const rows = $derived.by((): (readonly [string, string])[] => {
    if (!stats) return [];
    const { int } = app.i18n;
    const period =
      stats.first && stats.last
        ? `${app.i18n.dateTime(stats.first)} – ${app.i18n.dateTime(stats.last)}`
        : t('stats.none');
    return [
      [
        t('stats.messages'),
        t('stats.messagesValue', {
          total: int(stats.userMessages + stats.agentMessages),
          user: int(stats.userMessages),
          agent: int(stats.agentMessages),
        }),
      ],
      [
        t('stats.words'),
        t('stats.wordsValue', {
          total: int(stats.userWords + stats.agentWords),
          user: int(stats.userWords),
          agent: int(stats.agentWords),
        }),
      ],
      [t('stats.characters'), int(stats.characters)],
      [t('stats.reading'), t('stats.readingValue', { n: int(stats.readingMinutes) })],
      [
        t('stats.tokens'),
        t('stats.tokensValue', { input: int(stats.inputTokens), output: int(stats.outputTokens) }),
      ],
      [t('stats.cost'), stats.cost ? app.i18n.money(stats.cost) : t('stats.free')],
      [
        t('stats.latency'),
        stats.averageLatencyMs === null
          ? t('stats.none')
          : app.i18n.duration(stats.averageLatencyMs),
      ],
      [t('stats.tools'), int(stats.tools)],
      [t('stats.period'), period],
      [t('stats.branches'), int(stats.otherBranches)],
      [
        t('stats.models'),
        stats.models.length
          ? stats.models
              .map((m) => t('stats.modelsValue', { name: m.name, n: int(m.replies) }))
              .join('; ')
          : t('stats.none'),
      ],
    ];
  });

  async function copy() {
    try {
      await navigator.clipboard.writeText(statsMarkdown(t('stats.title'), rows));
      app.toasts.show({ kind: 'success', message: t('stats.copied') });
    } catch {
      app.toasts.show({ kind: 'error', message: t('msg.copyFailed') });
    }
  }
</script>

<Dialog.Root open onOpenChange={(open) => !open && closeStats()}>
  <Dialog.Portal>
    <Dialog.Overlay class="alfa-stats-overlay" />
    <Dialog.Content class="alfa-stats">
      <Dialog.Title class="alfa-stats-title">{t('stats.title')}</Dialog.Title>
      <Dialog.Description class="alfa-stats-lead">{t('stats.lead')}</Dialog.Description>
      {#if !stats}
        <p class="empty">{t('stats.empty')}</p>
      {:else}
        <dl class="list" aria-label={t('stats.list')}>
          {#each rows as [label, value] (label)}
            <div class="row">
              <dt>{label}</dt>
              <dd>{value}</dd>
            </div>
          {/each}
        </dl>
      {/if}
      <div class="footer">
        {#if stats}
          <Button variant="secondary" onclick={() => void copy()}>{t('stats.copy')}</Button>
        {/if}
        <Button variant="primary" onclick={closeStats}>{t('stats.close')}</Button>
      </div>
    </Dialog.Content>
  </Dialog.Portal>
</Dialog.Root>

<style>
  :global(.alfa-stats-overlay) {
    position: fixed;
    inset: 0;
    z-index: 110;
    background: var(--alfa-color-scrim);
  }
  :global(.alfa-stats) {
    position: fixed;
    z-index: 111;
    top: 50%;
    left: 50%;
    display: flex;
    flex-direction: column;
    gap: var(--alfa-space-3);
    width: min(600px, calc(var(--alfa-vw, 100vw) - 32px));
    max-height: min(calc(var(--alfa-vh, 100vh) * 0.85), 720px);
    padding: var(--alfa-space-6);
    transform: translate(-50%, -50%);
    border: 1px solid var(--alfa-color-border);
    border-radius: var(--alfa-radius-overlay);
    background: var(--alfa-color-surface);
    box-shadow: var(--alfa-shadow-3);
    color: var(--alfa-color-text);
  }
  :global(.alfa-stats-title) {
    margin: 0;
    font-size: var(--alfa-font-size-lg);
    font-weight: var(--alfa-weight-semibold);
  }
  :global(.alfa-stats-lead),
  .empty {
    margin: 0;
    color: var(--alfa-color-text-muted);
    font-size: var(--alfa-font-size-sm);
  }
  .list {
    display: flex;
    flex-direction: column;
    gap: var(--alfa-space-2);
    min-height: 0;
    margin: 0;
    overflow: auto;
  }
  .row {
    display: grid;
    grid-template-columns: minmax(140px, 40%) 1fr;
    gap: var(--alfa-space-3);
    padding-bottom: var(--alfa-space-2);
    border-bottom: 1px solid var(--alfa-color-border);
  }
  dt {
    color: var(--alfa-color-text-muted);
    font-size: var(--alfa-font-size-sm);
  }
  dd {
    margin: 0;
    font-size: var(--alfa-font-size-sm);
    overflow-wrap: anywhere;
  }
  .footer {
    display: flex;
    flex-wrap: wrap;
    justify-content: flex-end;
    gap: var(--alfa-space-2);
  }
</style>
