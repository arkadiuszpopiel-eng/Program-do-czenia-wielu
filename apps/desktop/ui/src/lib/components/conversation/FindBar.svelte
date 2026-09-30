<!-- Szukaj w rozmowie (Ctrl+F): przeszukuje dane gałęzi (nie DOM — lista jest wirtualizowana). -->
<script lang="ts">
  import { IconButton } from '@alfa/ui-kit';
  import ChevronUp from '@lucide/svelte/icons/chevron-up';
  import ChevronDown from '@lucide/svelte/icons/chevron-down';
  import X from '@lucide/svelte/icons/x';
  import type { Turn } from '../../api/types';
  import { normalize } from '../../logic/fuzzy';
  import { useApp } from '../../state/context';

  interface Props {
    path: readonly Turn[];
    focusTurn?: string | null;
  }

  let { path, focusTurn = $bindable(null) }: Props = $props();
  const app = useApp();
  const { t } = app.i18n;
  let query = $state('');
  let index = $state(0);

  const matches = $derived.by(() => {
    const q = normalize(query.trim());
    return q ? path.filter((turn) => normalize(turn.text).includes(q)).map((turn) => turn.id) : [];
  });

  $effect(() => {
    focusTurn = matches[Math.min(index, matches.length - 1)] ?? null;
  });

  function step(delta: number) {
    if (matches.length === 0) return;
    index = (index + delta + matches.length) % matches.length;
  }

  function key(event: KeyboardEvent) {
    if (event.key === 'Enter') {
      event.preventDefault();
      step(event.shiftKey ? -1 : 1);
    } else if (event.key === 'Escape') {
      event.preventDefault();
      event.stopPropagation();
      close();
    }
  }

  function close() {
    app.findOpen = false;
    focusTurn = null;
  }
</script>

<div class="find" role="search">
  <label class="alfa-visually-hidden" for="alfa-find-input">{t('conv.find')}</label>
  <input
    id="alfa-find-input"
    type="search"
    placeholder={t('conv.find')}
    bind:value={query}
    oninput={() => (index = 0)}
    onkeydown={key}
  />
  <span class="count" aria-live="polite">
    {#if query.trim()}
      {matches.length
        ? t('conv.findCount', { index: index + 1, total: matches.length })
        : t('conv.findNone')}
    {/if}
  </span>
  <IconButton
    label={t('conv.findPrev')}
    size="sm"
    onclick={() => step(-1)}
    disabled={!matches.length}
  >
    <ChevronUp size={14} strokeWidth={1.5} />
  </IconButton>
  <IconButton
    label={t('conv.findNext')}
    size="sm"
    onclick={() => step(1)}
    disabled={!matches.length}
  >
    <ChevronDown size={14} strokeWidth={1.5} />
  </IconButton>
  <IconButton label={t('conv.findClose')} size="sm" onclick={close}>
    <X size={14} strokeWidth={1.5} />
  </IconButton>
</div>

<style>
  .find {
    display: flex;
    align-items: center;
    gap: var(--alfa-space-1);
    padding: var(--alfa-space-1) var(--alfa-space-1) var(--alfa-space-1) var(--alfa-space-3);
    border: 1px solid var(--alfa-color-border);
    border-radius: var(--alfa-radius-card);
    background: var(--alfa-color-surface);
    box-shadow: var(--alfa-shadow-2);
  }
  input {
    flex: 1;
    min-width: 0;
    height: 28px;
    border: 0;
    background: transparent;
  }
  input:focus {
    outline: none;
  }
  .find:focus-within {
    outline: var(--alfa-size-focus-ring) solid var(--alfa-color-focus);
    outline-offset: 1px;
  }
  .count {
    color: var(--alfa-color-text-muted);
    font-size: var(--alfa-font-size-xs);
    font-variant-numeric: tabular-nums;
    white-space: nowrap;
  }
</style>
