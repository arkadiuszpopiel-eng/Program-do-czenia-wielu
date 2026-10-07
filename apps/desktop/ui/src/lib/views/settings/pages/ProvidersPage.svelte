<!-- Modele i dostawcy → Hub kont i kluczy (makieta 12): lista kont ze stanem, test, limit, usuń, kreator. -->
<script lang="ts">
  import { Button, ConfirmDialog, EmptyState } from '@alfa/ui-kit';
  import KeyRound from '@lucide/svelte/icons/key-round';
  import Plus from '@lucide/svelte/icons/plus';
  import type { Account, ProviderInfo } from '../../../api/types-hub';
  import LoadFailed from '../../../components/shell/LoadFailed.svelte';
  import Loading from '../../../components/shell/Loading.svelte';
  import { attempt, load } from '../../../state/attempt';
  import { useApp } from '../../../state/context';
  import AddProviderWizard from './AddProviderWizard.svelte';
  import BridgesSection from './BridgesSection.svelte';

  const app = useApp();
  const { t } = app.i18n;
  let accounts = $state<Account[]>([]);
  let catalog = $state<readonly ProviderInfo[]>([]);
  let loaded = $state(false);
  let loadError = $state<string | null>(null);
  let wizard = $state(false);
  let removing = $state<Account | null>(null);
  let confirmOpen = $state(false);

  $effect(() => {
    if (app.hubWizard) wizard = true;
  });

  /** Lista kont i katalog; błąd → „Nie udało się wczytać" z „Ponów" (nie mylący pusty stan). */
  async function reload(): Promise<boolean> {
    const result = await load(() =>
      Promise.all([app.client.accounts.list(), app.client.accounts.catalog()]),
    );
    loaded = true;
    if (result.status !== 'ready') {
      loadError = result.status === 'failed' ? result.error : null;
      return false;
    }
    const [list, cat] = result.value;
    accounts = [...list];
    catalog = cat;
    loadError = null;
    return true;
  }

  async function refreshSystem() {
    await attempt(app.toasts, async () => {
      app.system = await app.client.system.status();
    });
  }

  $effect(() => {
    void reload();
    return app.on((event) => {
      if (event.type !== 'AccountChanged') return;
      accounts = accounts.map((a) => (a.id === event.account.id ? event.account : a));
    });
  });

  const providerName = (id: string) => catalog.find((p) => p.id === id)?.display_name ?? id;

  async function test(account: Account) {
    accounts = accounts.map((a) => (a.id === account.id ? { ...a, state: 'testing' } : a));
    await attempt(app.toasts, () => app.client.accounts.test(account.id));
    // Bez świeżej listy cofamy „testowanie" ręcznie — inaczej spinner zostałby na zawsze.
    if (!(await reload())) accounts = accounts.map((a) => (a.id === account.id ? account : a));
  }

  async function remove() {
    // AlertDialog.Action (bits-ui) nie zamyka okna sam — zamykamy je przed wywołaniem rdzenia.
    confirmOpen = false;
    const target = removing;
    if (!target) return;
    const ok = await attempt(app.toasts, () => app.client.accounts.remove(target.id));
    removing = null;
    if (!ok) return;
    await reload();
    await refreshSystem();
  }
</script>

<section class="hub" aria-labelledby="hub-title">
  <div class="head">
    <div>
      <h3 id="hub-title">{t('hub.title')}</h3>
      <p class="desc">{t('hub.intro')}</p>
    </div>
    {#if !wizard}
      <Button variant="primary" onclick={() => (wizard = true)}>
        {#snippet icon()}<Plus size={14} strokeWidth={1.5} />{/snippet}
        {t('hub.add')}
      </Button>
    {/if}
  </div>
  {#if wizard}
    <AddProviderWizard
      onfinish={async () => {
        wizard = false;
        app.hubWizard = false;
        await reload();
        await refreshSystem();
      }}
    />
  {/if}
  {#if loadError}
    <LoadFailed error={loadError} onretry={() => void reload()} />
  {:else if !loaded}
    <Loading />
  {/if}
  {#if loaded && !loadError && accounts.length === 0 && !wizard}
    <EmptyState title={t('hub.accounts')} description={t('hub.empty')}>
      {#snippet icon()}<KeyRound size={20} strokeWidth={1.5} />{/snippet}
    </EmptyState>
  {:else if accounts.length}
    <ul class="accounts" aria-label={t('hub.accounts')}>
      {#each accounts as account (account.id)}
        <li class="account">
          <div class="info">
            <span class="name">{account.label}</span>
            <span class="meta">
              {providerName(account.provider_id)} · {t('hub.models', { n: account.models.length })} ·
              {account.cost_limit.enabled
                ? t('hub.limit', { amount: app.i18n.money(account.cost_limit.monthly) })
                : t('hub.noLimit')}
              {#if account.last_tested_at}· {t('hub.tested', {
                  when: app.i18n.relative(account.last_tested_at),
                })}{/if}
            </span>
          </div>
          <span class="state state-{account.state}">{t(`hub.state.${account.state}`)}</span>
          <Button
            size="sm"
            variant="secondary"
            loading={account.state === 'testing'}
            onclick={() => test(account)}>{t('hub.test')}</Button
          >
          <Button
            size="sm"
            variant="ghost"
            onclick={() => {
              removing = account;
              confirmOpen = true;
            }}
          >
            {t('hub.remove')}
          </Button>
        </li>
      {/each}
    </ul>
  {/if}
</section>

<BridgesSection />

<ConfirmDialog
  bind:open={confirmOpen}
  title={t('hub.removeTitle')}
  description={t('hub.removeConfirm', { label: removing?.label ?? '' })}
  confirmLabel={t('hub.remove')}
  cancelLabel={t('common.cancel')}
  danger
  onconfirm={remove}
/>

<style>
  .hub {
    display: flex;
    flex-direction: column;
    gap: var(--alfa-space-3);
    margin-bottom: var(--alfa-space-4);
  }
  .head {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: var(--alfa-space-4);
  }
  h3 {
    font-size: var(--alfa-font-size-md);
  }
  .desc,
  .meta {
    color: var(--alfa-color-text-muted);
    font-size: var(--alfa-font-size-sm);
  }
  .meta {
    font-size: var(--alfa-font-size-xs);
  }
  .accounts {
    display: flex;
    flex-direction: column;
    gap: var(--alfa-space-2);
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .account {
    display: flex;
    align-items: center;
    gap: var(--alfa-space-3);
    padding: var(--alfa-space-3);
    border: 1px solid var(--alfa-color-border);
    border-radius: var(--alfa-radius-card);
    background: var(--alfa-color-surface);
  }
  .info {
    display: flex;
    flex-direction: column;
    flex: 1;
    min-width: 0;
  }
  .name {
    font-weight: var(--alfa-weight-semibold);
  }
  .state {
    font-size: var(--alfa-font-size-xs);
    font-weight: var(--alfa-weight-semibold);
  }
  .state-ok {
    color: var(--alfa-color-success);
  }
  .state-invalid,
  .state-rate_limited {
    color: var(--alfa-color-error);
  }
  .state-unconfigured,
  .state-testing,
  .state-disabled {
    color: var(--alfa-color-text-muted);
  }
</style>
