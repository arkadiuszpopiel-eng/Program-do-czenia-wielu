<!--
  Jedna wersja w Historii zmian: wiersz z numerem, datą i licznikami wag oraz — po rozwinięciu —
  zdanie wstępu i pozycje w sekcjach. Waga jest podana kolorem i słowem.
-->
<script lang="ts">
  import type { ChangeLogEntry, ChangeSeverity } from '../../../api/types-updates';
  import { SEVERITIES, countBySeverity, groupBySection } from '../../../logic/change-history';
  import { useApp } from '../../../state/context';

  interface Props {
    entry: ChangeLogEntry;
    expanded: boolean;
    /** Identyfikator rozwijanej treści (unikalny także, gdy lista jest na stronie i w oknie). */
    panelId: string;
    ontoggle: () => void;
  }

  let { entry, expanded, panelId, ontoggle }: Props = $props();
  const { t, tk } = useApp().i18n;

  const counts = $derived(countBySeverity([entry]));
  const label = (severity: ChangeSeverity) => tk(`history.severity.${severity}`);
</script>

<li class="version" class:expanded>
  <button
    type="button"
    class="row"
    aria-expanded={expanded}
    aria-controls={panelId}
    onclick={ontoggle}
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
    <div class="body" id={panelId}>
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

<style>
  .version {
    --sev-critical: var(--alfa-color-error);
    --sev-serious: var(--alfa-color-warning);
    --sev-normal: var(--alfa-color-success);
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
    border-radius: var(--alfa-radius-control);
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
