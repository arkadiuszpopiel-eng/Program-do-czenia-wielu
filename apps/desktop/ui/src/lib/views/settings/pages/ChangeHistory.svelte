<!--
  Ustawienia → Aktualizacje → Historia zmian: wszystkie wersje z dziennika zmian wbudowanego
  w program, od najnowszej. Każda pozycja ma wagę — krytyczna (czerwona), poważna (pomarańczowa),
  zwykła (zielona) — pokazaną kolorem i słowem (kolor nie jest jedynym nośnikiem informacji).
  Filtr zostawia pozycje wybranej wagi; wersje bez takich pozycji znikają z listy.
  Główny widok pokazuje dziesięć ostatnich wersji; starsze są w zakładce „Starsze” — dojście
  z zakładki u góry i z przycisku pod listą.
  Tryb osadzony (`embedded`, okno „Co nowego”): bez karty i nagłówka, z gotową listą wpisów
  (`preloaded`) — ta sama lista, te same zakładki.
-->
<script lang="ts">
  import type { ChangeLogEntry } from '../../../api/types-updates';
  import ChangeHistoryVersion from './ChangeHistoryVersion.svelte';
  import LoadFailed from '../../../components/shell/LoadFailed.svelte';
  import {
    SEVERITIES,
    countBySeverity,
    filterHistory,
    splitHistory,
    type HistoryTab,
    type SeverityFilter,
  } from '../../../logic/change-history';
  import { load } from '../../../state/attempt';
  import { useApp } from '../../../state/context';

  interface Props {
    /** Bez karty i nagłówka (w oknie „Co nowego”). */
    embedded?: boolean;
    /** Wpisy wczytane przez rodzica — komponent nie pyta rdzenia drugi raz. */
    preloaded?: readonly ChangeLogEntry[] | null;
  }

  let { embedded = false, preloaded = null }: Props = $props();
  const uid = $props.id();
  const app = useApp();
  const { t, tk } = app.i18n;

  let entries = $state<readonly ChangeLogEntry[] | null>(null);
  let loadError = $state<string | null>(null);
  let filter = $state<SeverityFilter>('all');
  let tab = $state<HistoryTab>('recent');
  let tabsEl = $state<HTMLElement | null>(null);
  /** Rozwinięte wersje; na starcie najnowsza. */
  let open = $state<Record<string, boolean>>({});

  function show(list: readonly ChangeLogEntry[]) {
    entries = list;
    loadError = null;
    const first = list[0];
    if (first) open = { [first.version]: true };
  }

  async function reload() {
    const result = await load(() => app.client.updates.history());
    if (result.status === 'ready') show(result.value);
    else if (result.status === 'failed') loadError = result.error;
  }

  $effect(() => {
    if (preloaded) show(preloaded);
    else void reload();
  });

  const totals = $derived(countBySeverity(entries ?? []));
  const split = $derived(splitHistory(entries ?? []));
  const hasOlder = $derived(split.older.length > 0);
  const lists = $derived({
    recent: filterHistory(split.recent, filter),
    older: filterHistory(split.older, filter),
  });
  const shown = $derived(lists[hasOlder ? tab : 'recent']);
  const allOpen = $derived(shown.length > 0 && shown.every((e) => open[e.version]));

  function toggle(version: string) {
    open = { ...open, [version]: !open[version] };
  }

  function toggleAll() {
    open = allOpen ? {} : Object.fromEntries(shown.map((e) => [e.version, true]));
  }

  /** Zmiana zakładki; `focus` przenosi fokus na nową zakładkę (klawiatura, przycisk pod listą). */
  function selectTab(next: HistoryTab, focus = false) {
    tab = next;
    if (focus)
      queueMicrotask(() => tabsEl?.querySelector<HTMLElement>(`[data-tab='${next}']`)?.focus());
  }

  function onTabKey(event: KeyboardEvent) {
    if (event.key !== 'ArrowLeft' && event.key !== 'ArrowRight') return;
    event.preventDefault();
    selectTab(tab === 'recent' ? 'older' : 'recent', true);
  }
</script>

<svelte:element
  this={embedded ? 'div' : 'section'}
  class={embedded ? 'history embedded' : 'wk-card history'}
  aria-labelledby={embedded ? undefined : 'history-title'}
>
  <div class="head">
    <div>
      {#if !embedded}
        <h3 id="history-title">{t('history.title')}</h3>
        <p class="hint">{t('history.hint')}</p>
      {/if}
    </div>
    {#if shown.length > 1}
      <button type="button" class="link" onclick={toggleAll}>
        {allOpen ? t('history.collapseAll') : t('history.expandAll')}
      </button>
    {/if}
  </div>

  {#if loadError}<LoadFailed error={loadError} onretry={() => void reload()} />{/if}

  {#if entries}
    <div class="filters" role="group" aria-label={t('history.filter')}>
      <button
        type="button"
        class="chip"
        aria-pressed={filter === 'all'}
        onclick={() => (filter = 'all')}
      >
        {t('history.all')} <span class="count">{totals.all}</span>
      </button>
      {#each SEVERITIES as severity (severity)}
        <button
          type="button"
          class="chip"
          aria-pressed={filter === severity}
          onclick={() => (filter = severity)}
        >
          <span class="dot {severity}" aria-hidden="true"></span>
          {tk(`history.filter.${severity}`)} <span class="count">{totals[severity]}</span>
        </button>
      {/each}
    </div>

    {#if hasOlder}
      <div class="tabs" role="tablist" aria-label={t('history.tabs')} bind:this={tabsEl}>
        {#each ['recent', 'older'] as const as key (key)}
          <button
            type="button"
            role="tab"
            class="tab"
            id="{uid}-tab-{key}"
            data-tab={key}
            aria-selected={tab === key}
            aria-controls="{uid}-panel"
            tabindex={tab === key ? 0 : -1}
            onclick={() => selectTab(key)}
            onkeydown={onTabKey}
          >
            {tk(`history.tab.${key}`)} <span class="count">{lists[key].length}</span>
          </button>
        {/each}
      </div>
    {/if}

    {#if shown.length === 0}
      <p class="empty">{hasOlder ? t('history.emptyTab') : t('history.empty')}</p>
    {/if}

    <div
      id="{uid}-panel"
      role={hasOlder ? 'tabpanel' : undefined}
      aria-labelledby={hasOlder ? `${uid}-tab-${tab}` : undefined}
    >
      <ul class="versions">
        {#each shown as entry (entry.version)}
          <ChangeHistoryVersion
            {entry}
            panelId="{uid}-{entry.version}"
            expanded={Boolean(open[entry.version])}
            ontoggle={() => toggle(entry.version)}
          />
        {/each}
      </ul>
    </div>

    {#if hasOlder}
      <div class="more">
        {#if tab === 'recent'}
          <button type="button" class="link" onclick={() => selectTab('older', true)}>
            {t('history.showOlder', { n: split.older.length })}
          </button>
        {:else}
          <button type="button" class="link" onclick={() => selectTab('recent', true)}>
            {t('history.showRecent')}
          </button>
        {/if}
      </div>
    {/if}
  {/if}
</svelte:element>

<style>
  .history {
    --sev-critical: var(--alfa-color-error);
    --sev-serious: var(--alfa-color-warning);
    --sev-normal: var(--alfa-color-success);
  }
  .embedded .filters {
    margin-top: 0;
  }
  .embedded .head {
    justify-content: flex-end;
    margin-bottom: var(--alfa-space-2);
  }
  .head {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: var(--alfa-space-3);
  }
  .hint,
  .empty {
    margin: var(--alfa-space-1) 0 0;
    color: var(--alfa-color-text-muted);
    font-size: var(--alfa-font-size-sm);
  }
  .link {
    flex: none;
    padding: var(--alfa-space-1) var(--alfa-space-2);
    border: 1px solid var(--alfa-color-border);
    border-radius: var(--alfa-radius-control);
    background: transparent;
    color: var(--alfa-color-text);
    font: inherit;
    font-size: var(--alfa-font-size-sm);
    cursor: pointer;
  }
  .link:hover {
    background: var(--alfa-color-surface2);
  }
  .filters {
    display: flex;
    flex-wrap: wrap;
    gap: var(--alfa-space-2);
    margin: var(--alfa-space-3) 0;
  }
  .chip {
    display: inline-flex;
    align-items: center;
    gap: var(--alfa-space-2);
    min-height: 28px;
    padding: 0 var(--alfa-space-3);
    border: 1px solid var(--alfa-color-border);
    border-radius: var(--alfa-radius-full);
    background: transparent;
    color: var(--alfa-color-text);
    font: inherit;
    font-size: var(--alfa-font-size-sm);
    cursor: pointer;
  }
  .chip:hover {
    background: var(--alfa-color-surface2);
  }
  .chip[aria-pressed='true'] {
    border-color: var(--alfa-color-focus);
    background: var(--alfa-color-surface3);
    font-weight: var(--alfa-weight-semibold);
  }
  .count {
    color: var(--alfa-color-text-muted);
    font-variant-numeric: tabular-nums;
  }
  .tabs {
    display: flex;
    gap: var(--alfa-space-1);
    margin-bottom: var(--alfa-space-2);
    border-bottom: 1px solid var(--alfa-color-border);
  }
  .tab {
    display: inline-flex;
    align-items: center;
    gap: var(--alfa-space-2);
    min-height: 32px;
    margin-bottom: -1px;
    padding: 0 var(--alfa-space-3);
    border: 0;
    border-bottom: 2px solid transparent;
    border-radius: var(--alfa-radius-control) var(--alfa-radius-control) 0 0;
    background: transparent;
    color: var(--alfa-color-text-muted);
    font: inherit;
    font-size: var(--alfa-font-size-sm);
    cursor: pointer;
  }
  .tab:hover {
    background: var(--alfa-color-surface2);
    color: var(--alfa-color-text);
  }
  .tab[aria-selected='true'] {
    border-bottom-color: var(--alfa-color-focus);
    color: var(--alfa-color-text);
    font-weight: var(--alfa-weight-semibold);
  }
  .more {
    display: flex;
    justify-content: center;
    padding-top: var(--alfa-space-3);
    border-top: 1px solid var(--alfa-color-border);
  }
  .dot {
    flex: none;
    width: 8px;
    height: 8px;
    border-radius: 50%;
  }
  .dot.critical {
    background: var(--sev-critical);
  }
  /* Kropka „poważna” ma być pomarańczowa także w jasnym motywie — kolor ostrzeżenia jest tam
     przyciemniony pod czytelność tekstu i wychodzi brązowy. Tekst etykiety zostaje przy nim. */
  .dot.serious {
    background: light-dark(#dd6b00, var(--sev-serious));
  }
  .dot.normal {
    background: var(--sev-normal);
  }
  .versions {
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .tabs + div > .versions > :global(.version:first-child) {
    border-top: 0;
  }
</style>
