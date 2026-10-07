<!-- Dziennik zmian zakresu pamięci (Ty / Strażniczka) z cofaniem odwracalnych zmian. -->
<script lang="ts">
  import { Button } from '@alfa/ui-kit';
  import type { MemoryJournalEntry } from '../../api/types-memory';
  import { attempt, load } from '../../state/attempt';
  import { useApp } from '../../state/context';
  import LoadFailed from '../shell/LoadFailed.svelte';

  interface Props {
    scope: string;
    onchange?: () => void;
  }

  let { scope, onchange }: Props = $props();
  const app = useApp();
  const { t } = app.i18n;
  let entries = $state<readonly MemoryJournalEntry[]>([]);
  let loadError = $state<string | null>(null);

  // Błąd = „Nie udało się wczytać" z „Ponów", nie mylące „dziennik jest pusty".
  async function loadEntries() {
    const key = scope;
    const result = await load(() => app.client.memory.journal(key));
    if (key !== scope) return;
    if (result.status === 'ready') {
      entries = result.value;
      loadError = null;
    } else if (result.status === 'failed') {
      loadError = result.error;
    }
  }

  $effect(() => {
    void scope;
    void loadEntries();
  });

  async function undo(entry: MemoryJournalEntry) {
    const ok = await attempt(app.toasts, async () => {
      const result = await app.client.memory.undo(scope, entry.id);
      app.toasts.show({
        kind: 'success',
        message: t('memory.undoDone', { restored: result.restored, removed: result.removed }),
      });
    });
    if (!ok) return;
    await loadEntries();
    onchange?.();
  }
</script>

<details class="journal">
  <summary>{t('memory.journal')}</summary>
  {#if loadError}
    <LoadFailed error={loadError} onretry={() => void loadEntries()} />
  {:else if entries.length === 0}
    <p class="meta">{t('memory.journalEmpty')}</p>
  {:else}
    <ul aria-label={t('memory.journalScope', { scope })}>
      {#each entries as entry (entry.id)}
        <li>
          <span class="meta"
            >{app.i18n.relative(entry.at)} · {entry.run
              ? t('memory.byKeeper')
              : t('memory.byUser')}</span
          >
          <span class:undone={entry.undone}>{entry.note}</span>
          {#if entry.undone}
            <span class="meta">({t('memory.undone')})</span>
          {:else if entry.undoable}
            <Button size="sm" variant="ghost" onclick={() => undo(entry)}>{t('memory.undo')}</Button
            >
          {/if}
        </li>
      {/each}
    </ul>
  {/if}
</details>

<style>
  .journal {
    font-size: var(--alfa-font-size-sm);
  }
  summary {
    cursor: pointer;
    font-weight: var(--alfa-weight-semibold);
  }
  ul {
    display: flex;
    flex-direction: column;
    gap: var(--alfa-space-1);
    margin: var(--alfa-space-1) 0 0;
    padding: 0;
    list-style: none;
  }
  li {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--alfa-space-1);
  }
  .meta {
    color: var(--alfa-color-text-muted);
    font-size: var(--alfa-font-size-xs);
  }
  .undone {
    text-decoration: line-through;
  }
</style>
