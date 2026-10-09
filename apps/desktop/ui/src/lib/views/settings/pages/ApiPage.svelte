<!--
  Ustawienia → Kolejki modeli API (zgłoszenie właściciela, 2026-10-09: konto ze 133 modelami,
  a wybrać nie dało się żadnego). Dla każdego konta API: kolejka do dziesięciu modeli na klasę
  zadań i przeglądarka pełnej listy modeli dostawcy. Klucze i stan kont — „Konta i klucze API”.
-->
<script lang="ts">
  import { Banner, Button, EmptyState } from '@alfa/ui-kit';
  import ListOrdered from '@lucide/svelte/icons/list-ordered';
  import type { Account, ProviderInfo } from '../../../api/types-hub';
  import LoadFailed from '../../../components/shell/LoadFailed.svelte';
  import Loading from '../../../components/shell/Loading.svelte';
  import { errorText } from '../../../api/command-error';
  import { MAX_QUEUE, fitsClass, type TaskClassId } from '../../../logic/model-queue';
  import { load } from '../../../state/attempt';
  import { useApp } from '../../../state/context';
  import ApiModelBrowser from './ApiModelBrowser.svelte';
  import ApiQueueEditor from './ApiQueueEditor.svelte';

  const app = useApp();
  const { t } = app.i18n;
  const uid = $props.id();

  let accounts = $state<Account[]>([]);
  let catalog = $state<readonly ProviderInfo[]>([]);
  let loaded = $state(false);
  let loadError = $state<string | null>(null);
  let taskClass = $state<TaskClassId>('chat');
  let saving = $state(false);

  const account = $derived(accounts.find((a) => a.id === app.apiAccount) ?? accounts[0]);
  const providerName = (a: Account) =>
    catalog.find((p) => p.id === a.provider_id)?.display_name ?? a.provider_id;
  const queue = $derived(account?.queues?.[taskClass] ?? []);
  const chatModels = $derived(
    account ? account.models.filter((m) => fitsClass('chat', m.purpose)).length : 0,
  );

  async function reload() {
    const result = await load(() =>
      Promise.all([app.client.accounts.list(), app.client.accounts.catalog()]),
    );
    loaded = true;
    if (result.status !== 'ready') {
      loadError = result.status === 'failed' ? result.error : null;
      return;
    }
    [accounts, catalog] = [[...result.value[0]], result.value[1]];
    loadError = null;
  }

  $effect(() => {
    void reload();
    return app.on((event) => {
      if (event.type !== 'AccountChanged') return;
      accounts = accounts.map((a) => (a.id === event.account.id ? event.account : a));
    });
  });

  /** Zapis kolejki bieżącej klasy; odmowa rdzenia (np. model nie do tej klasy) → komunikat. */
  async function save(models: string[]): Promise<boolean> {
    const current = account;
    if (!current || saving) return false;
    saving = true;
    try {
      const updated = await app.client.accounts.setQueue(current.id, taskClass, models);
      accounts = accounts.map((a) => (a.id === updated.id ? updated : a));
      return true;
    } catch (error) {
      app.toasts.show({ kind: 'error', message: errorText(error) });
      return false;
    } finally {
      saving = false;
    }
  }

  function add(id: string) {
    if (queue.length < MAX_QUEUE && !queue.includes(id)) void save([...queue, id]);
  }
</script>

<div class="page">
  <p class="intro">{t('api.intro')}</p>

  {#if loadError}
    <LoadFailed error={loadError} onretry={() => void reload()} />
  {:else if !loaded}
    <Loading />
  {:else if !account}
    <EmptyState title={t('api.empty')} description={t('api.emptyHint')}>
      {#snippet icon()}<ListOrdered size={20} strokeWidth={1.5} />{/snippet}
      <Button variant="primary" onclick={() => app.openSettings('providers')}
        >{t('api.goProviders')}</Button
      >
    </EmptyState>
  {:else}
    <div class="accounts" role="tablist" aria-label={t('api.accounts')}>
      {#each accounts as a (a.id)}
        <button
          type="button"
          role="tab"
          class="account"
          id="{uid}-acc-{a.id}"
          aria-selected={a.id === account.id}
          aria-controls="{uid}-account"
          onclick={() => (app.apiAccount = a.id)}
        >
          <span class="label">{a.label}</span>
          <span class="meta">
            {t('api.accountMeta', { provider: providerName(a), n: a.models.length })}
          </span>
        </button>
      {/each}
    </div>

    <div
      class="selected"
      role="tabpanel"
      id="{uid}-account"
      aria-labelledby="{uid}-acc-{account.id}"
    >
      <p class="summary">
        {t('api.summary.chat', { n: chatModels })} ·
        {account.auto_model
          ? t('api.summary.auto', { model: account.auto_model })
          : t('api.summary.noAuto')}
      </p>
      <div>
        <Button variant="ghost" size="sm" onclick={() => app.openSettings('providers')}
          >{t('api.toAccount')}</Button
        >
      </div>
      {#if account.state === 'disabled'}
        <Banner kind="warning" icon="warning">{t('api.disabled')}</Banner>
      {/if}

      <section class="card" aria-labelledby="{uid}-queues">
        <h3 id="{uid}-queues">{t('api.queues')}</h3>
        <p class="muted">{t('api.queuesHint')}</p>
        <ApiQueueEditor {account} bind:taskClass onsave={save} {saving} />
      </section>

      <section class="card" aria-labelledby="{uid}-browser">
        <h3 id="{uid}-browser">{t('api.browser')}</h3>
        {#if account.models.length === 0}
          <p class="muted">{t('api.noModels')}</p>
        {:else}
          <ApiModelBrowser models={account.models} {queue} {taskClass} {saving} onadd={add} />
        {/if}
      </section>
    </div>
  {/if}
</div>

<style>
  .page {
    display: flex;
    flex-direction: column;
    gap: var(--alfa-space-4);
  }
  .intro,
  .muted,
  .summary {
    margin: 0;
    color: var(--alfa-color-text-muted);
    font-size: var(--alfa-font-size-sm);
  }
  .accounts {
    display: flex;
    flex-wrap: wrap;
    gap: var(--alfa-space-2);
  }
  .account {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 2px;
    min-width: 200px;
    padding: var(--alfa-space-3);
    border: 1px solid var(--alfa-color-border);
    border-radius: var(--alfa-radius-card);
    background: var(--alfa-color-surface);
    color: var(--alfa-color-text);
    font: inherit;
    text-align: start;
    cursor: pointer;
  }
  .account:hover {
    border-color: var(--alfa-color-border-strong);
  }
  .account[aria-selected='true'] {
    border-color: var(--alfa-color-accent);
    box-shadow: inset 0 0 0 1px var(--alfa-color-accent);
  }
  .account:focus-visible {
    outline: var(--alfa-size-focus-ring) solid var(--alfa-color-focus);
    outline-offset: 2px;
  }
  .label {
    font-weight: var(--alfa-weight-semibold);
  }
  .meta {
    color: var(--alfa-color-text-muted);
    font-size: var(--alfa-font-size-xs);
  }
  .selected {
    display: flex;
    flex-direction: column;
    gap: var(--alfa-space-4);
  }
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
    margin: 0;
    font-size: var(--alfa-font-size-lg);
  }
</style>
