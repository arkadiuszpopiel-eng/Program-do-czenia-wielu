<!-- Modele i dostawcy → Hub kont i kluczy (makieta 12): lista kont ze stanem, test, limit, usuń, kreator. -->
<script lang="ts">
  import { Button, ConfirmDialog, EmptyState } from '@alfa/ui-kit';
  import KeyRound from '@lucide/svelte/icons/key-round';
  import Plus from '@lucide/svelte/icons/plus';
  import type { Account, ProviderInfo } from '../../../api/types-hub';
  import { useApp } from '../../../state/context';
  import AddProviderWizard from './AddProviderWizard.svelte';

  const app = useApp();
  const { t } = app.i18n;
  let accounts = $state<Account[]>([]);
  let catalog = $state<readonly ProviderInfo[]>([]);
  let wizard = $state(false);
  let removing = $state<Account | null>(null);
  let confirmOpen = $state(false);

  $effect(() => {
    if (app.hubWizard) wizard = true;
  });

  async function load() {
    const [list, cat] = await Promise.all([
      app.client.accounts.list(),
      app.client.accounts.catalog(),
    ]);
    accounts = [...list];
    catalog = cat;
  }

  $effect(() => {
    void load();
    return app.on((event) => {
      if (event.type !== 'AccountChanged') return;
      accounts = accounts.map((a) => (a.id === event.account.id ? event.account : a));
    });
  });

  const providerName = (id: string) => catalog.find((p) => p.id === id)?.display_name ?? id;

  async function test(account: Account) {
    accounts = accounts.map((a) => (a.id === account.id ? { ...a, state: 'testing' } : a));
    await app.client.accounts.test(account.id);
    await load();
  }

  async function remove() {
    if (!removing) return;
    await app.client.accounts.remove(removing.id);
    removing = null;
    await load();
    app.system = await app.client.system.status();
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
        await load();
        app.system = await app.client.system.status();
      }}
    />
  {/if}
  {#if accounts.length === 0 && !wizard}
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
