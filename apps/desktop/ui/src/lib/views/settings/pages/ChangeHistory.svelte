<!--
  Ustawienia → Aktualizacje → Historia zmian: wszystkie wersje z dziennika zmian wbudowanego
  w program, od najnowszej. Każda pozycja ma wagę — krytyczna (czerwona), poważna (pomarańczowa),
  zwykła (zielona) — pokazaną kolorem i słowem (kolor nie jest jedynym nośnikiem informacji).
  Filtr zostawia pozycje wybranej wagi; wersje bez takich pozycji znikają z listy.
-->
<script lang="ts">
  import type { ChangeLogEntry, ChangeSeverity } from '../../../api/types-updates';
  import LoadFailed from '../../../components/shell/LoadFailed.svelte';
  import {
    SEVERITIES,
    countBySeverity,
    filterHistory,
    groupBySection,
    type SeverityFilter,
  } from '../../../logic/change-history';
  import { load } from '../../../state/attempt';
  import { useApp } from '../../../state/context';

  const app = useApp();
  const { t, tk } = app.i18n;

  let entries = $state<readonly ChangeLogEntry[] | null>(null);
  let loadError = $state<string | null>(null);
  let filter = $state<SeverityFilter>('all');
  /** Rozwinięte wersje; na starcie najnowsza. */
  let open = $state<Record<string, boolean>>({});

  async function reload() {
    const result = await load(() => app.client.updates.history());
    if (result.status === 'ready') {
      entries = result.value;
      loadError = null;
      const first = result.value[0];
      if (first) open = { [first.version]: true };
    } else if (result.status === 'failed') loadError = result.error;
  }

  $effect(() => {
    void reload();
  });

  const totals = $derived(countBySeverity(entries ?? []));
  const shown = $derived(filterHistory(entries ?? [], filter));
  const allOpen = $derived(shown.length > 0 && shown.every((e) => open[e.version]));

  function toggle(version: string) {
    open = { ...open, [version]: !open[version] };
  }

  function toggleAll() {
    open = allOpen ? {} : Object.fromEntries(shown.map((e) => [e.version, true]));
  }

  const label = (severity: ChangeSeverity) => tk(`history.severity.${severity}`);
</script>

<section class="wk-card history" aria-labelledby="history-title">
  <div class="head">
    <div>
      <h3 id="history-title">{t('history.title')}</h3>
      <p class="hint">{t('history.hint')}</p>
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

    {#if shown.length === 0}
      <p class="empty">{t('history.empty')}</p>
    {/if}

    <ul class="versions">
      {#each shown as entry (entry.version)}
        {@const counts = countBySeverity([entry])}
        {@const expanded = Boolean(open[entry.version])}
        <li class="version" class:expanded>
          <button
            type="button"
            class="row"
            aria-expanded={expanded}
            aria-controls="history-{entry.version}"
            onclick={() => toggle(entry.version)}
          >
            <span class="chevron" aria-hidden="true"></span>
            <span class="name">{t('history.version', { version: entry.version })}</span>
            {#if entry.date}<span class="date">{entry.date}</span>{/if}
            <span class="badges">
              {#each SEVERITIES as severity (severity)}
                {#if counts[severity] > 0}
                  <span
                    class="badge {severity}"
                    title={t('history.badge', { n: counts[severity], kind: label(severity) })}
                  >
                    <span class="dot {severity}" aria-hidden="true"></span>
                    <span aria-hidden="true">{counts[severity]}</span>
                    <span class="sr"
                      >{t('history.badge', { n: counts[severity], kind: label(severity) })}</span
                    >
                  </span>
                {/if}
              {/each}
            </span>
          </button>
          {#if expanded}
            <div class="body" id="history-{entry.version}">
              {#if entry.summary}<p class="summary">{entry.summary}</p>{/if}
              {#each groupBySection(entry.items) as group (group.section)}
                <h4>{group.section}</h4>
                <ul class="items">
                  {#each group.items as item, index (index)}
                    <li class="item {item.severity}">
                      <span class="mark {item.severity}">
                        <span class="dot {item.severity}" aria-hidden="true"></span>
                        {label(item.severity)}
                      </span>
                      <span class="text">{item.text}</span>
                    </li>
                  {/each}
                </ul>
              {/each}
            </div>
          {/if}
        </li>
      {/each}
    </ul>
  {/if}
</section>

<style>
  .history {
    --sev-critical: var(--alfa-color-error);
    --sev-serious: var(--alfa-color-warning);
    --sev-normal: var(--alfa-color-success);
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
    border-radius: var(--alfa-radius-sm);
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
  .versions,
  .items {
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .version {
    border-top: 1px solid var(--alfa-color-border);
  }
  .row {
    display: flex;
    align-items: center;
    gap: var(--alfa-space-3);
    width: 100%;
    min-height: 40px;
    padding: var(--alfa-space-2) var(--alfa-space-1);
    border: 0;
    border-radius: var(--alfa-radius-sm);
    background: transparent;
    color: var(--alfa-color-text);
    font: inherit;
    text-align: start;
    cursor: pointer;
  }
  .row:hover {
    background: var(--alfa-color-surface2);
  }
  .chevron {
    flex: none;
    width: 7px;
    height: 7px;
    border-right: 1.5px solid currentColor;
    border-bottom: 1.5px solid currentColor;
    transform: rotate(-45deg);
    transition: transform 120ms ease;
  }
  .expanded .chevron {
    transform: rotate(45deg);
  }
  :global([data-motion='off']) .chevron {
    transition: none;
  }
  .name {
    font-weight: var(--alfa-weight-semibold);
  }
  .date {
    color: var(--alfa-color-text-muted);
    font-size: var(--alfa-font-size-sm);
    font-variant-numeric: tabular-nums;
  }
  .badges {
    display: inline-flex;
    gap: var(--alfa-space-2);
    margin-inline-start: auto;
  }
  .badge {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    font-size: var(--alfa-font-size-sm);
    font-variant-numeric: tabular-nums;
  }
  .sr {
    position: absolute;
    width: 1px;
    height: 1px;
    overflow: hidden;
    clip-path: inset(50%);
    white-space: nowrap;
  }
  .body {
    padding: 0 var(--alfa-space-1) var(--alfa-space-3) calc(var(--alfa-space-3) + 11px);
  }
  .summary {
    margin: 0 0 var(--alfa-space-2);
  }
  h4 {
    margin: var(--alfa-space-3) 0 var(--alfa-space-1);
    color: var(--alfa-color-text-muted);
    font-size: var(--alfa-font-size-xs);
    font-weight: var(--alfa-weight-semibold);
    letter-spacing: 0.04em;
    text-transform: uppercase;
  }
  .item {
    display: grid;
    grid-template-columns: 104px 1fr;
    gap: var(--alfa-space-3);
    align-items: baseline;
    padding: var(--alfa-space-1) 0;
    font-size: var(--alfa-font-size-sm);
  }
  .mark {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    font-size: var(--alfa-font-size-xs);
    font-weight: var(--alfa-weight-semibold);
  }
  .mark.critical {
    color: var(--sev-critical);
  }
  .mark.serious {
    color: var(--sev-serious);
  }
  .mark.normal {
    color: var(--sev-normal);
  }
  @media (max-width: 560px) {
    .item {
      grid-template-columns: 1fr;
      gap: 2px;
    }
  }
</style>
