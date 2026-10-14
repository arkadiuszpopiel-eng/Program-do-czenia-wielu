<!--
  Ustawienia → Kolejki modeli API → „Wszystkie konta” (zgłoszenie właściciela 2026-10-14): jedna
  kolejność prób dla klasy zadań z modelami z różnych kont. Ma pierwszeństwo przed kolejkami kont;
  pusta klasa — działają kolejki kont i wybór automatyczny. Każda zmiana zapisuje się od razu.
-->
<script lang="ts">
  import { Button, Select } from '@alfa/ui-kit';
  import Layers from '@lucide/svelte/icons/layers';
  import type { Account, SharedQueueEntry, SharedQueues } from '../../../api/types-hub';
  import { errorText } from '../../../api/command-error';
  import LoadFailed from '../../../components/shell/LoadFailed.svelte';
  import { MAX_QUEUE, TASK_CLASSES, type TaskClassId } from '../../../logic/model-queue';
  import { entryKey, entryModels, entryOf, fromAccountQueues } from '../../../logic/shared-queue';
  import { load } from '../../../state/attempt';
  import { useApp } from '../../../state/context';
  import ApiModelBrowser from './ApiModelBrowser.svelte';
  import QueueClassTabs from './QueueClassTabs.svelte';
  import QueueTiles from './QueueTiles.svelte';

  interface Props {
    accounts: readonly Account[];
    /** Nazwa konta w kafelkach („OpenRouter”, „OpenAI — praca”). */
    label: (account: Account) => string;
  }

  let { accounts, label }: Props = $props();
  const app = useApp();
  const { t, tk } = app.i18n;
  const uid = $props.id();

  let queues = $state<SharedQueues>({});
  let failed = $state<string | null>(null);
  let taskClass = $state<TaskClassId>('chat');
  let saving = $state(false);
  let source = $state('');

  const keys = $derived((queues[taskClass] ?? []).map(entryKey));
  const models = $derived(entryModels(keys, accounts));
  const className = $derived(tk(`api.class.${taskClass}`));
  const picked = $derived(accounts.find((a) => a.id === source) ?? accounts[0]);
  /** Dla przeglądarki modeli: pozycje wybranego konta po nazwie modelu, reszta jako zajęte miejsca. */
  const browserQueue = $derived(
    keys.map((key, at) => {
      const entry = entryOf(key);
      return entry && entry.account_id === picked?.id ? entry.model : `\u0000${at}`;
    }),
  );
  const counts = $derived(
    Object.fromEntries(TASK_CLASSES.map((id) => [id, queues[id]?.length ?? 0])),
  );

  async function reload() {
    const result = await load(() => app.client.accounts.sharedQueues());
    if (result.status === 'ready') {
      queues = result.value;
      failed = null;
    } else if (result.status === 'failed') failed = result.error;
  }

  $effect(() => {
    void reload();
  });

  async function save(entries: readonly SharedQueueEntry[]): Promise<boolean> {
    if (saving) return false;
    saving = true;
    try {
      queues = await app.client.accounts.setSharedQueue(taskClass, entries);
      return true;
    } catch (error) {
      app.toasts.show({ kind: 'error', message: errorText(error) });
      return false;
    } finally {
      saving = false;
    }
  }

  const saveKeys = (next: string[]) =>
    save(next.map(entryOf).filter((e): e is SharedQueueEntry => e !== null));

  function add(model: string) {
    if (!picked || keys.length >= MAX_QUEUE) return;
    const key = entryKey({ account_id: picked.id, model });
    if (!keys.includes(key)) void saveKeys([...keys, key]);
  }

  async function compose() {
    const entries = fromAccountQueues(accounts, taskClass);
    if (entries.length === 0) {
      app.toasts.show({ kind: 'warning', message: t('api.shared.composeEmpty') });
      return;
    }
    if (await save(entries))
      app.toasts.show({ kind: 'success', message: t('api.shared.composed', { name: className }) });
  }

  async function clear() {
    if (await save([]))
      app.toasts.show({ kind: 'success', message: t('api.shared.cleared', { name: className }) });
  }

  /** Model u dostawcy (pod nazwą na kafelku). */
  const code = (key: string) => entryOf(key)?.model ?? key;
  /** Konto pozycji (wiersz nad nazwą na kafelku). */
  function owner(key: string): string {
    const account = accounts.find((a) => a.id === entryOf(key)?.account_id);
    return account ? label(account) : t('api.shared.gone');
  }
</script>

{#if failed}
  <LoadFailed error={failed} onretry={() => void reload()} />
{:else}
  <section class="card" aria-labelledby="{uid}-title">
    <h3 id="{uid}-title">
      <Layers size={18} strokeWidth={1.75} aria-hidden="true" />{t('api.shared.title')}
    </h3>
    <p class="muted">{t('api.shared.hint')}</p>
    <QueueClassTabs bind:value={taskClass} prefix={uid} {counts} />
    <div class="panel" role="tabpanel" id="{uid}-panel" aria-labelledby="{uid}-tab-{taskClass}">
      <p class="muted">{tk(`api.classHint.${taskClass}`)}</p>
      <div class="head">
        <h4>{t('api.shared.queue', { name: className })}</h4>
        <span class="muted">{t('api.queueCount', { n: keys.length, max: MAX_QUEUE })}</span>
      </div>
      {#if keys.length === 0}
        <p class="empty">
          <strong>{t('api.shared.empty')}</strong>
          {t('api.shared.emptyHint')}
        </p>
      {:else}
        <QueueTiles queue={keys} {models} {saving} onsave={saveKeys} {code} {owner} />
      {/if}
      <div class="actions">
        <Button variant="secondary" size="sm" disabled={saving} onclick={() => void compose()}
          >{t('api.shared.compose')}</Button
        >
        {#if keys.length > 0}
          <Button variant="ghost" size="sm" disabled={saving} onclick={() => void clear()}
            >{t('api.clear')}</Button
          >
        {/if}
      </div>
      <p class="muted small">{t('api.shared.oneRoute')}</p>
    </div>
  </section>

  <section class="card" aria-labelledby="{uid}-add">
    <h3 id="{uid}-add">{t('api.shared.add')}</h3>
    <label class="field">
      <span>{t('api.shared.fromAccount')}</span>
      <Select
        value={picked?.id ?? ''}
        onchange={(value) => (source = value)}
        label={t('api.shared.fromAccount')}
        options={accounts.map((a) => ({ value: a.id, label: label(a) }))}
      />
    </label>
    {#if picked && picked.models.length > 0}
      <ApiModelBrowser
        models={picked.models}
        queue={browserQueue}
        {taskClass}
        {saving}
        onadd={add}
      />
    {:else}
      <p class="muted">{t('api.noModels')}</p>
    {/if}
  </section>
{/if}

<style>
  .card {
    display: flex;
    flex-direction: column;
    gap: var(--alfa-space-3);
    padding: var(--alfa-space-4);
    border: 1px solid var(--alfa-color-border);
    border-radius: var(--alfa-radius-card);
    background: var(--alfa-color-surface);
  }
  h3 {
    display: flex;
    align-items: center;
    gap: var(--alfa-space-2);
    margin: 0;
    font-size: var(--alfa-font-size-lg);
  }
  h4 {
    margin: 0;
    font-size: var(--alfa-font-size-md);
  }
  .muted {
    margin: 0;
    color: var(--alfa-color-text-muted);
    font-size: var(--alfa-font-size-sm);
  }
  .small {
    font-size: var(--alfa-font-size-xs);
  }
  .panel {
    display: flex;
    flex-direction: column;
    gap: var(--alfa-space-3);
  }
  .head {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    gap: var(--alfa-space-3);
  }
  .empty {
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
  .field {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: var(--alfa-space-1);
    font-size: var(--alfa-font-size-sm);
    font-weight: var(--alfa-weight-semibold);
  }
</style>
