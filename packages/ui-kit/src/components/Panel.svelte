<script lang="ts">
  import type { Snippet } from 'svelte';
  import X from '@lucide/svelte/icons/x';
  import IconButton from './IconButton.svelte';

  interface Tab {
    readonly id: string;
    readonly label: string;
  }

  interface Props {
    title: string;
    /** Karty panelu (jeden panel naraz, przełączany kartami — §14.1). */
    tabs?: readonly Tab[];
    /** Aktywna karta (bindable). */
    activeTab?: string;
    onclose?: () => void;
    /** Dodatkowe akcje w nagłówku (przyciski ikon). */
    actions?: Snippet;
    children?: Snippet;
    /** Szerokość w px; panel prawy 300–520 px. */
    width?: number;
    side?: 'left' | 'right';
    closeLabel?: string;
    tabsLabel?: string;
    /** Ukryj nagłówek z tytułem (np. gdy karty same nazywają panel). */
    hideHeader?: boolean;
  }

  let {
    title,
    tabs = [],
    activeTab = $bindable(tabs[0]?.id ?? ''),
    onclose,
    actions,
    children,
    width,
    side = 'right',
    closeLabel,
    tabsLabel,
    hideHeader = false,
  }: Props = $props();

  const tablistId = $props.id();

  function onTabKeydown(event: KeyboardEvent, index: number) {
    if (
      event.key !== 'ArrowRight' &&
      event.key !== 'ArrowLeft' &&
      event.key !== 'Home' &&
      event.key !== 'End'
    )
      return;
    event.preventDefault();
    const n = tabs.length;
    let next = index;
    if (event.key === 'ArrowRight') next = (index + 1) % n;
    if (event.key === 'ArrowLeft') next = (index - 1 + n) % n;
    if (event.key === 'Home') next = 0;
    if (event.key === 'End') next = n - 1;
    const tab = tabs[next];
    if (!tab) return;
    activeTab = tab.id;
    const tabsEl = (event.currentTarget as HTMLElement).parentElement;
    const tabButtons = tabsEl?.querySelectorAll<HTMLElement>('[role="tab"]');
    tabButtons?.[next]?.focus();
  }
</script>

<section class="panel {side}" style:width={width ? `${width}px` : undefined} aria-label={title}>
  {#snippet headActions()}
    <div class="actions">
      {#if actions}{@render actions()}{/if}
      {#if onclose}
        <IconButton label={closeLabel ?? `Zamknij panel ${title}`} size="sm" onclick={onclose}>
          <X size={16} strokeWidth={1.5} />
        </IconButton>
      {/if}
    </div>
  {/snippet}
  {#if hideHeader}
    <h2 class="alfa-visually-hidden">{title}</h2>
  {:else}
    <header class="head">
      <h2 class="title">{title}</h2>
      {@render headActions()}
    </header>
  {/if}
  {#if tabs.length > 0}
    <div class="tabs-row">
      <div class="tabs" role="tablist" aria-label={tabsLabel ?? `Karty panelu ${title}`}>
        {#each tabs as tab, i (tab.id)}
          <button
            type="button"
            role="tab"
            id="{tablistId}-tab-{tab.id}"
            class="tab"
            aria-selected={activeTab === tab.id}
            aria-controls="{tablistId}-panel"
            tabindex={activeTab === tab.id ? 0 : -1}
            onclick={() => (activeTab = tab.id)}
            onkeydown={(e) => onTabKeydown(e, i)}
          >
            {tab.label}
          </button>
        {/each}
      </div>
      {#if hideHeader}{@render headActions()}{/if}
    </div>
  {/if}
  <div
    class="body"
    id="{tablistId}-panel"
    role={tabs.length > 0 ? 'tabpanel' : undefined}
    aria-labelledby={tabs.length > 0 ? `${tablistId}-tab-${activeTab}` : undefined}
  >
    {#if children}{@render children()}{/if}
  </div>
</section>

<style>
  .panel {
    position: relative;
    display: flex;
    flex-direction: column;
    min-width: 0;
    height: 100%;
    background: var(--alfa-color-surface);
    color: var(--alfa-color-text);
  }
  .right {
    border-left: 1px solid var(--alfa-color-border);
  }
  .left {
    border-right: 1px solid var(--alfa-color-border);
  }
  .head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--alfa-space-2);
    height: 40px;
    padding: 0 var(--alfa-space-2) 0 var(--alfa-space-4);
  }
  .title {
    font-size: var(--alfa-font-size-xs);
    font-weight: var(--alfa-weight-semibold);
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: var(--alfa-color-text-muted);
  }
  .actions {
    display: inline-flex;
    gap: var(--alfa-space-1);
  }
  .tabs-row {
    display: flex;
    align-items: center;
    border-bottom: 1px solid var(--alfa-color-border);
  }
  .tabs-row > .actions {
    padding: 0 var(--alfa-space-1);
  }
  .tabs {
    display: flex;
    flex: 1;
    min-width: 0;
    gap: var(--alfa-space-1);
    padding: 0 var(--alfa-space-2);
    overflow-x: auto;
    scrollbar-width: none;
  }
  .tab {
    position: relative;
    min-height: 32px;
    padding: 0 var(--alfa-space-2);
    border: 0;
    background: transparent;
    color: var(--alfa-color-text-muted);
    font-size: var(--alfa-font-size-sm);
    font-weight: var(--alfa-weight-semibold);
    white-space: nowrap;
  }
  .tab[aria-selected='true'] {
    color: var(--alfa-color-text);
  }
  .tab[aria-selected='true']::after {
    content: '';
    position: absolute;
    left: var(--alfa-space-2);
    right: var(--alfa-space-2);
    bottom: -1px;
    height: 2px;
    border-radius: 1px;
    background: var(--alfa-color-text);
  }
  .body {
    flex: 1;
    min-height: 0;
    overflow: auto;
    padding: var(--alfa-space-3) var(--alfa-space-4);
  }
</style>
