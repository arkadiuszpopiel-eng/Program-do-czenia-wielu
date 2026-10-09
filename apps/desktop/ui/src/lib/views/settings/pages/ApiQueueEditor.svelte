<!--
  Kolejki modeli konta API: klasy zadań (z licznikiem pozycji) i kolejka wybranej klasy — kolejność
  prób Routera. Każda zmiana zapisuje się od razu; pusta kolejka to wybór automatyczny.
-->
<script lang="ts">
  import { Button, Menu, TextField, type MenuItem } from '@alfa/ui-kit';
  import Sparkles from '@lucide/svelte/icons/sparkles';
  import type { Account } from '../../../api/types-hub';
  import {
    MAX_QUEUE,
    STRATEGIES,
    TASK_CLASSES,
    fitsClass,
    type QueueStrategy,
    type TaskClassId,
  } from '../../../logic/model-queue';
  import { useApp } from '../../../state/context';
  import QueueTiles from './QueueTiles.svelte';

  interface Props {
    account: Account;
    taskClass: TaskClassId;
    /** Zapis kolejki bieżącej klasy; `false` = rdzeń odmówił (lista zostaje bez zmian). */
    onsave: (models: string[]) => Promise<boolean>;
    saving: boolean;
  }

  let { account, taskClass = $bindable(), onsave, saving }: Props = $props();
  const app = useApp();
  const { t, tk } = app.i18n;
  const uid = $props.id();

  let manual = $state('');
  const queue = $derived(account.queues?.[taskClass] ?? []);
  const className = $derived(tk(`api.class.${taskClass}`));
  const byId = $derived(new Map(account.models.map((m) => [m.id, m])));
  /** Model używany bez kolejki: automatyczny wybór rdzenia, o ile pasuje do tej klasy. */
  const auto = $derived(fitsClass(taskClass, 'chat') ? (account.auto_model ?? null) : null);
  const full = $derived(queue.length >= MAX_QUEUE);
  /** Ile modeli konta pasuje do tej klasy (dla opisu pustej kolejki). */
  const fitting = $derived(account.models.filter((m) => fitsClass(taskClass, m.purpose)).length);

  async function suggest(strategy: QueueStrategy) {
    try {
      const models = await app.client.accounts.suggestQueue(account.id, taskClass, strategy);
      if (models.length === 0) {
        app.toasts.show({ kind: 'warning', message: t('api.suggestEmpty') });
        return;
      }
      if (await onsave(models))
        app.toasts.show({
          kind: 'success',
          message: t('api.suggested', {
            name: className,
            strategy: tk(`api.strategy.${strategy}`),
          }),
        });
    } catch (error) {
      app.toasts.show({
        kind: 'error',
        message: error instanceof Error ? error.message : String(error),
      });
    }
  }

  async function clear() {
    if (await onsave([]))
      app.toasts.show({ kind: 'success', message: t('api.cleared', { name: className }) });
  }

  async function addManual(event: SubmitEvent) {
    event.preventDefault();
    const id = manual.trim();
    if (!id || full || queue.includes(id)) return;
    if (await onsave([...queue, id])) manual = '';
  }

  const strategies = $derived<MenuItem[]>(
    STRATEGIES.map((strategy) => ({
      id: strategy,
      label: tk(`api.strategy.${strategy}`),
      onSelect: () => void suggest(strategy),
    })),
  );
</script>

<div class="classes" role="tablist" aria-label={t('api.classes')}>
  {#each TASK_CLASSES as id (id)}
    {@const n = account.queues?.[id]?.length ?? 0}
    <button
      type="button"
      role="tab"
      id="{uid}-tab-{id}"
      class="class"
      aria-selected={taskClass === id}
      aria-controls="{uid}-panel"
      tabindex={taskClass === id ? 0 : -1}
      onclick={() => (taskClass = id)}
      onkeydown={(event) => {
        const at = TASK_CLASSES.indexOf(taskClass);
        const step = event.key === 'ArrowRight' ? 1 : event.key === 'ArrowLeft' ? -1 : 0;
        const next = TASK_CLASSES[(at + step + TASK_CLASSES.length) % TASK_CLASSES.length];
        if (!step || !next) return;
        event.preventDefault();
        taskClass = next;
        document.getElementById(`${uid}-tab-${next}`)?.focus();
      }}
    >
      <span>{tk(`api.class.${id}`)}</span>
      <span class="count" class:set={n > 0}>{n > 0 ? n : t('api.autoShort')}</span>
    </button>
  {/each}
</div>

<div class="panel" role="tabpanel" id="{uid}-panel" aria-labelledby="{uid}-tab-{taskClass}">
  <p class="hint">{tk(`api.classHint.${taskClass}`)}</p>
  <div class="head">
    <h4>{t('api.queue', { name: className })}</h4>
    <span class="muted">{t('api.queueCount', { n: queue.length, max: MAX_QUEUE })}</span>
  </div>

  {#if queue.length === 0}
    <p class="auto">
      <strong>{t('api.auto')}.</strong>
      {auto
        ? t('api.autoHint', { model: auto })
        : fitting > 0
          ? t('api.autoOther')
          : t('api.autoNone')}
    </p>
  {:else}
    <QueueTiles {queue} models={byId} {saving} {onsave} />
  {/if}

  <div class="actions">
    <Menu items={strategies} label={t('api.suggest')} align="start">
      {#snippet trigger(props)}
        <Button variant="secondary" size="sm" disabled={saving} {...props}>
          <Sparkles size={14} strokeWidth={1.75} aria-hidden="true" />
          {t('api.suggest')}
        </Button>
      {/snippet}
    </Menu>
    {#if queue.length > 0}
      <Button variant="ghost" size="sm" disabled={saving} onclick={clear}>{t('api.clear')}</Button>
    {/if}
  </div>

  <form class="manual" onsubmit={addManual}>
    <TextField
      label={t('api.manual')}
      hint={t('api.manualHint')}
      bind:value={manual}
      autocomplete="off"
      spellcheck={false}
    />
    <Button
      variant="secondary"
      size="sm"
      type="submit"
      disabled={!manual.trim() || full || saving || queue.includes(manual.trim())}
      >{t('api.manualAdd')}</Button
    >
  </form>
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
  .panel {
    display: flex;
    flex-direction: column;
    gap: var(--alfa-space-3);
  }
  .hint,
  .muted {
    margin: 0;
    color: var(--alfa-color-text-muted);
    font-size: var(--alfa-font-size-sm);
  }
  .head {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    gap: var(--alfa-space-3);
  }
  h4 {
    margin: 0;
    font-size: var(--alfa-font-size-md);
  }
  .auto {
    margin: 0;
    padding: var(--alfa-space-3) var(--alfa-space-4);
    border: 1px dashed var(--alfa-color-border-strong);
    border-radius: var(--alfa-radius-control);
    font-size: var(--alfa-font-size-sm);
  }
  .actions {
    display: flex;
    flex-wrap: wrap;
    gap: var(--alfa-space-2);
  }
  .manual {
    display: flex;
    flex-wrap: wrap;
    align-items: flex-start;
    gap: var(--alfa-space-2);
  }
  .manual > :global(:first-child) {
    flex: 1 1 280px;
  }
  .manual > :global(button) {
    margin-top: 1.6em;
  }
</style>
