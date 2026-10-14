<!--
  Klasy zadań jako zakładki z licznikiem pozycji kolejki (kolejka konta i kolejka wspólna).
  Strzałki w lewo i prawo przechodzą między klasami.
-->
<script lang="ts">
  import { TASK_CLASSES, type TaskClassId } from '../../../logic/model-queue';
  import { useApp } from '../../../state/context';

  interface Props {
    value: TaskClassId;
    /** Liczba pozycji kolejki każdej klasy (brak — kolejka pusta). */
    counts: Partial<Record<TaskClassId, number>>;
    /** Przedrostek identyfikatorów: zakładki `<prefix>-tab-<klasa>`, panel `<prefix>-panel`. */
    prefix: string;
  }

  let { value = $bindable(), counts, prefix }: Props = $props();
  const app = useApp();
  const { t, tk } = app.i18n;
  const tabId = (id: TaskClassId) => `${prefix}-tab-${id}`;
</script>

<div class="classes" role="tablist" aria-label={t('api.classes')}>
  {#each TASK_CLASSES as id (id)}
    {@const n = counts[id] ?? 0}
    <button
      type="button"
      role="tab"
      id={tabId(id)}
      class="class"
      aria-selected={value === id}
      aria-controls="{prefix}-panel"
      tabindex={value === id ? 0 : -1}
      onclick={() => (value = id)}
      onkeydown={(event) => {
        const at = TASK_CLASSES.indexOf(value);
        const step = event.key === 'ArrowRight' ? 1 : event.key === 'ArrowLeft' ? -1 : 0;
        const next = TASK_CLASSES[(at + step + TASK_CLASSES.length) % TASK_CLASSES.length];
        if (!step || !next) return;
        event.preventDefault();
        value = next;
        document.getElementById(tabId(next))?.focus();
      }}
    >
      <span>{tk(`api.class.${id}`)}</span>
      <span class="count" class:set={n > 0}>{n > 0 ? n : t('api.autoShort')}</span>
    </button>
  {/each}
</div>

<style>
  .classes {
    display: flex;
    flex-wrap: wrap;
    gap: var(--alfa-space-2);
  }
  .class {
    display: inline-flex;
    align-items: center;
    gap: var(--alfa-space-2);
    padding: var(--alfa-space-2) var(--alfa-space-3);
    border: 1px solid var(--alfa-color-border);
    border-radius: var(--alfa-radius-control);
    background: var(--alfa-color-surface);
    color: var(--alfa-color-text);
    font: inherit;
    font-size: var(--alfa-font-size-sm);
    cursor: pointer;
  }
  .class:hover {
    border-color: var(--alfa-color-border-strong);
  }
  .class[aria-selected='true'] {
    border-color: var(--alfa-color-accent);
    background: var(--alfa-color-surface2);
    font-weight: var(--alfa-weight-semibold);
  }
  .class:focus-visible {
    outline: var(--alfa-size-focus-ring) solid var(--alfa-color-focus);
    outline-offset: 2px;
  }
  .count {
    color: var(--alfa-color-text-muted);
    font-size: var(--alfa-font-size-xs);
    font-weight: var(--alfa-weight-regular);
  }
  .count.set {
    min-width: 1.5em;
    padding: 0 var(--alfa-space-1);
    border-radius: var(--alfa-radius-full);
    border: 1px solid var(--alfa-color-border-strong);
    color: var(--alfa-color-text);
    font-weight: var(--alfa-weight-semibold);
    text-align: center;
  }
</style>
