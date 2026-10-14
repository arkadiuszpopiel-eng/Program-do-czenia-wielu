<!--
  Wiersz konta w „Konta i klucze API": stan z powodem ostatniego testu (nie samo „klucz odrzucony”),
  wyjaśnienie z krokami naprawy, wymiana klucza bez usuwania konta (stary klucz zostaje, dopóki nowy
  nie przejdzie testu), wyłączanie i usuwanie.
-->
<script lang="ts">
  import { Button } from '@alfa/ui-kit';
  import { errorText } from '../../../api/command-error';
  import type { Account, ProviderInfo, TestReport } from '../../../api/types-hub';
  import {
    actionFor,
    cleanKey,
    keyAgeDays,
    keyRotationDue,
    rotateStores,
    verdictOf,
  } from '../../../logic/api-key';
  import { attempt, showError } from '../../../state/attempt';
  import { useApp } from '../../../state/context';
  import WizardKeyStep from './WizardKeyStep.svelte';
  import WizardTestResult from './WizardTestResult.svelte';

  interface Props {
    account: Account;
    provider: ProviderInfo | undefined;
    onchange: () => Promise<unknown>;
    onremove: (account: Account) => void;
  }

  let { account, provider, onchange, onremove }: Props = $props();
  const app = useApp();
  const { t, tk } = app.i18n;

  /** Rozwinięty panel pod wierszem: wyjaśnienie ostatniego testu albo wymiana klucza. */
  let panel = $state<'none' | 'why' | 'rotate'>('none');
  let testing = $state(false);
  let saving = $state(false);
  let toggling = $state(false);
  let secret = $state('');
  let rotateError = $state('');
  /** Wynik testu nowego klucza (przy porażce stary klucz zostaje w koncie). */
  let rotated = $state<TestReport | null>(null);

  const last = $derived(account.last_test ?? null);
  /** Raport do wyjaśnienia: z ostatniego testu zapisanego przy koncie. */
  const lastReport = $derived<TestReport | null>(
    last
      ? {
          ok: last.ok,
          latency_ms: last.latency_ms,
          models: account.models,
          error: null,
          reason: last.reason === 'ok' ? null : last.reason,
          status: last.status ?? null,
          provider_code: last.provider_code ?? null,
          detail: last.detail ?? null,
        }
      : null,
  );
  const verdict = $derived(lastReport ? verdictOf(lastReport) : null);
  const disabled = $derived(account.state === 'disabled');
  /** Wiek klucza w dniach i przypomnienie o wymianie (ustawienie `accounts.key_max_age_days`). */
  const keyAge = $derived(keyAgeDays(account.key_saved_at, Date.now()));
  const keyOld = $derived(keyRotationDue(keyAge, app.num('accounts.key_max_age_days', 180)));
  const busy = $derived(testing || account.state === 'testing');
  /** Konto bez klucza w magazynie: ostatni test niczego już nie opisuje. */
  const keyless = $derived(account.state === 'unconfigured' || !account.key_stored);
  /** Stan ogólny zamiast powodu ostatniego testu. */
  const plain = $derived(disabled || busy || keyless || !verdict);
  /** Czy wymiana zapisała nowy klucz (reguła rdzenia). */
  const rotatedStored = $derived(rotated ? rotateStores(verdictOf(rotated)) : false);
  /** Etykieta stanu: wyłączone / w trakcie / bez klucza / powód ostatniego testu / stan ogólny. */
  const stateLabel = $derived(
    plain || !verdict
      ? t(`hub.state.${busy ? 'testing' : keyless && !disabled ? 'unconfigured' : account.state}`)
      : tk(`hub.reason.${verdict}`),
  );
  const tone = $derived(
    plain || !verdict
      ? account.state === 'ok' && !plain
        ? 'good'
        : 'muted'
      : verdict === 'ok'
        ? 'good'
        : actionFor(verdict) === 'works' || verdict === 'unsupported'
          ? 'warn'
          : 'bad',
  );
  /** Jest co wyjaśniać: test się nie udał albo udał z zastrzeżeniem. */
  const explainable = $derived(Boolean(verdict && verdict !== 'ok' && !disabled && !keyless));

  async function test() {
    testing = true;
    await attempt(app.toasts, () => app.client.accounts.test(account.id));
    await onchange();
    testing = false;
  }

  async function rotate() {
    const key = cleanKey(secret);
    if (!key || saving) return;
    saving = true;
    rotateError = '';
    try {
      rotated = await app.client.accounts.rotate(account.id, key);
      secret = '';
      await onchange();
    } catch (error) {
      rotateError = errorText(error);
    } finally {
      saving = false;
    }
  }

  async function toggle() {
    toggling = true;
    await attempt(app.toasts, () => app.client.accounts.setDisabled(account.id, !disabled));
    await onchange();
    toggling = false;
  }

  async function setModel(model: string) {
    try {
      const updated = await app.client.accounts.setModel(account.id, model);
      // Wynik wymiany pokazuje własną kopię listy modeli — bez tego formularz wracałby pusty.
      if (rotated) rotated = { ...rotated, models: updated.models };
    } catch (error) {
      showError(app.toasts, error);
    }
    await onchange();
  }

  /** Ponowny test klucza zapisanego przy wymianie (np. po doładowaniu konta). */
  async function retestRotated() {
    saving = true;
    try {
      rotated = await app.client.accounts.test(account.id);
    } catch (error) {
      showError(app.toasts, error);
    } finally {
      saving = false;
    }
    await onchange();
  }

  function show(next: 'why' | 'rotate') {
    panel = panel === next ? 'none' : next;
    rotated = null;
    rotateError = '';
    secret = '';
  }
</script>

<li class="account" class:off={disabled}>
  <div class="row">
    <div class="info">
      <span class="name">{account.label}</span>
      <span class="meta">
        {provider?.display_name ?? account.provider_id} · {t('hub.models', {
          n: account.models.length,
        })} ·
        {account.cost_limit.enabled
          ? t('hub.limit', { amount: app.i18n.money(account.cost_limit.monthly) })
          : t('hub.noLimit')}
        {#if account.last_tested_at}· {t('hub.tested', {
            when: app.i18n.relative(account.last_tested_at),
          })}{/if}
        {#if keyAge !== null && !keyOld}· {t('hub.keyAge', { n: keyAge })}{/if}
      </span>
      {#if keyOld && keyAge !== null}
        <span class="meta old">{t('hub.keyOld', { n: keyAge })}</span>
      {/if}
      {#if last?.ok && last.detail}
        <span class="meta">{t('hub.testNote', { note: last.detail })}</span>
      {/if}
    </div>
    <span class="state {tone}">{stateLabel}</span>
    <div class="actions">
      {#if explainable}
        <Button
          size="sm"
          variant="ghost"
          aria-expanded={panel === 'why'}
          onclick={() => show('why')}>{t('hub.why')}</Button
        >
      {/if}
      <Button size="sm" variant="secondary" loading={busy} {disabled} onclick={test}
        >{t('hub.test')}</Button
      >
      {#if account.models.length > 0}
        <Button size="sm" variant="ghost" onclick={() => app.openApiFor(account.id)}
          >{t('hub.toModels')}</Button
        >
      {/if}
      {#if provider?.auth === 'api_key'}
        <Button
          size="sm"
          variant="ghost"
          aria-expanded={panel === 'rotate'}
          onclick={() => show('rotate')}>{t('hub.rotate')}</Button
        >
      {/if}
      <Button size="sm" variant="ghost" loading={toggling} onclick={toggle}
        >{disabled ? t('hub.enable') : t('hub.disable')}</Button
      >
      <Button size="sm" variant="ghost" onclick={() => onremove(account)}>{t('hub.remove')}</Button>
    </div>
  </div>
  {#if panel === 'why' && lastReport}
    <div class="panel">
      <WizardTestResult
        report={lastReport}
        testing={busy}
        providerId={account.provider_id}
        providerName={provider?.display_name ?? account.provider_id}
        busy={false}
        fixLabel={t('hub.rotate')}
        nextLabel={t('common.close')}
        onnext={() => (panel = 'none')}
        onretry={test}
        onfix={() => show('rotate')}
        onmodel={setModel}
      />
    </div>
  {:else if panel === 'rotate' && provider}
    <div class="panel">
      {#if rotated}
        <WizardTestResult
          report={rotated}
          testing={saving}
          providerId={account.provider_id}
          providerName={provider.display_name}
          busy={false}
          note={rotatedStored ? t('hub.rotated') : t('hub.rotateKept')}
          onretry={rotatedStored ? retestRotated : undefined}
          fixLabel={t('hub.rotateOther')}
          nextLabel={t('common.close')}
          onnext={() => (panel = 'none')}
          onclose={() => (panel = 'none')}
          onfix={() => (rotated = null)}
          onmodel={setModel}
        />
      {:else}
        <WizardKeyStep
          {provider}
          bind:secret
          label=""
          baseUrl=""
          {saving}
          error={rotateError}
          title={t('hub.rotateTitle', { label: account.label })}
          submitLabel={t('hub.rotateSubmit')}
          rotate
          onsubmit={rotate}
        />
      {/if}
    </div>
  {/if}
</li>

<style>
  .account {
    display: flex;
    flex-direction: column;
    gap: var(--alfa-space-3);
    padding: var(--alfa-space-3);
    border: 1px solid var(--alfa-color-border);
    border-radius: var(--alfa-radius-card);
    background: var(--alfa-color-surface);
  }
  .row {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--alfa-space-2) var(--alfa-space-3);
  }
  .info {
    display: flex;
    flex-direction: column;
    flex: 1 1 240px;
    min-width: 0;
  }
  .name {
    font-weight: var(--alfa-weight-semibold);
  }
  .off .name {
    color: var(--alfa-color-text-muted);
  }
  .meta {
    color: var(--alfa-color-text-muted);
    font-size: var(--alfa-font-size-xs);
  }
  .meta.old {
    color: var(--alfa-color-warning);
  }
  .state {
    font-size: var(--alfa-font-size-xs);
    font-weight: var(--alfa-weight-semibold);
  }
  .good {
    color: var(--alfa-color-success);
  }
  .warn {
    color: var(--alfa-color-warning);
  }
  .bad {
    color: var(--alfa-color-error);
  }
  .muted {
    color: var(--alfa-color-text-muted);
  }
  .actions {
    display: flex;
    flex-wrap: wrap;
    gap: var(--alfa-space-1);
  }
  .panel {
    display: flex;
    flex-direction: column;
    gap: var(--alfa-space-3);
    padding-top: var(--alfa-space-3);
    border-top: 1px solid var(--alfa-color-border);
  }
</style>
