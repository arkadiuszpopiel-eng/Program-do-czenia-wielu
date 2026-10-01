<!-- Dziennik zmian zakresu pamięci (Ty / Strażniczka) z cofaniem odwracalnych zmian. -->
<script lang="ts">
  import { Button } from '@alfa/ui-kit';
  import type { MemoryJournalEntry } from '../../api/types-memory';
  import { useApp } from '../../state/context';

  interface Props {
    scope: string;
    onchange?: () => void;
  }

  let { scope, onchange }: Props = $props();
  const app = useApp();
  const { t } = app.i18n;
  let entries = $state<readonly MemoryJournalEntry[]>([]);

  async function load() {
    entries = await app.client.memory.journal(scope);
  }

  $effect(() => {
    void scope;
    void load();
  });

  async function undo(entry: MemoryJournalEntry) {
    try {
      const result = await app.client.memory.undo(scope, entry.id);
      app.toasts.show({
        kind: 'success',
        message: t('memory.undoDone', { restored: result.restored, removed: result.removed }),
      });
      await load();
      onchange?.();
    } catch (error) {
      app.toasts.show({
        kind: 'error',
        message: error instanceof Error ? error.message : String(error),
      });
    }
  }
</script>

<details class="journal">
  <summary>{t('memory.journal')}</summary>
  {#if entries.length === 0}
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
