<!-- Lista wiadomości aktywnej gałęzi z separatorami dni, „↓ nowe (n)" i skokiem do wyniku wyszukiwania. -->
<script lang="ts">
  import ArrowDown from '@lucide/svelte/icons/arrow-down';
  import type { Turn } from '../../api/types';
  import { dayKey } from '../../i18n/format';
  import { now } from '../../state/clock.svelte';
  import { useApp } from '../../state/context';
  import type { ConversationState } from '../../state/conversation.svelte';
  import MessageItem from './MessageItem.svelte';
  import VirtualList from './VirtualList.svelte';

  interface Props {
    conv: ConversationState;
    /** Id tury podświetlonej (wynik wyszukiwania). */
    focusTurn?: string | null;
  }

  let { conv, focusTurn = null }: Props = $props();
  const app = useApp();
  const { t } = app.i18n;

  type Row =
    { kind: 'day'; key: string; label: string } | { kind: 'turn'; key: string; turn: Turn };

  let list = $state<ReturnType<typeof VirtualList> | null>(null);
  let atBottom = $state(true);
  let newBelow = $state(0);

  function dayLabel(iso: string): string {
    const today = dayKey(new Date(now()).toISOString());
    const yesterday = dayKey(new Date(now() - 86_400_000).toISOString());
    const key = dayKey(iso);
    if (key === today) return t('conv.today');
    if (key === yesterday) return t('conv.yesterday');
    return app.i18n.day(iso);
  }

  const rows = $derived.by((): Row[] => {
    const out: Row[] = [];
    let last = '';
    for (const turn of conv.path) {
      const key = dayKey(turn.created_at);
      if (key !== last) {
        out.push({ kind: 'day', key: `day:${key}`, label: dayLabel(turn.created_at) });
        last = key;
      }
      out.push({ kind: 'turn', key: turn.id, turn });
    }
    return out;
  });
  const turnCount = $derived(conv.path.length);
  const positions = $derived(Object.fromEntries(conv.path.map((turn, i) => [turn.id, i + 1])));

  $effect(() => {
    if (!focusTurn || !list) return;
    const index = rows.findIndex((r) => r.key === focusTurn);
    if (index >= 0) list.scrollToIndex(index);
  });
</script>

<div class="wrap">
  <VirtualList
    bind:this={list}
    bind:atBottom
    bind:newBelow
    items={rows}
    keyOf={(row) => row.key}
    label={t('conv.messages')}
    busy={Boolean(conv.streaming)}
  >
    {#snippet row(item)}
      <div class="column">
        {#if item.kind === 'day'}
          <!-- Separator dnia jest wizualny; data jest w etykiecie każdej wiadomości. -->
          <div class="day" aria-hidden="true"><span>{item.label}</span></div>
        {:else}
          <MessageItem
            turn={item.turn}
            {conv}
            position={positions[item.turn.id] ?? 0}
            count={turnCount}
            highlighted={item.turn.id === focusTurn}
          />
        {/if}
      </div>
    {/snippet}
  </VirtualList>
  {#if !atBottom}
    <button type="button" class="new" onclick={() => list?.scrollToBottom()}>
      <ArrowDown size={14} strokeWidth={1.5} aria-hidden="true" />
      {newBelow > 0 ? t('conv.newBelow', { n: newBelow }) : t('conv.toBottom')}
    </button>
  {/if}
</div>

<style>
  .wrap {
    position: relative;
    flex: 1;
    min-height: 0;
  }
  .column {
    width: min(var(--conv-column, var(--alfa-size-reading-column)), 100%);
    margin: 0 auto;
    padding: 0 var(--alfa-space-4);
  }
  .day {
    display: flex;
    align-items: center;
    gap: var(--alfa-space-3);
    padding: var(--alfa-space-4) 0 var(--alfa-space-2);
    color: var(--alfa-color-text-subtle);
    font-size: var(--alfa-font-size-xs);
    font-weight: var(--alfa-weight-semibold);
  }
  .day::before,
  .day::after {
    content: '';
    flex: 1;
    height: 1px;
    background: var(--alfa-color-border);
  }
  .new {
    position: absolute;
    left: 50%;
    bottom: var(--alfa-space-3);
    display: inline-flex;
    align-items: center;
    gap: var(--alfa-space-1);
    min-height: 32px;
    padding: 0 var(--alfa-space-3);
    transform: translateX(-50%);
    border: 1px solid var(--alfa-color-border);
    border-radius: var(--alfa-radius-full);
    background: var(--alfa-color-surface);
    box-shadow: var(--alfa-shadow-2);
    color: var(--alfa-color-text);
    font-size: var(--alfa-font-size-sm);
    font-weight: var(--alfa-weight-semibold);
  }
</style>
