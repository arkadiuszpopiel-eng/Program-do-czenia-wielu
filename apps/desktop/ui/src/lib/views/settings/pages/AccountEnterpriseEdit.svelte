<!--
  Wiersz konta → „Ustawienia dla firm” (C2.12, wersja 0.0.65): zmiana organizacji i projektu OpenAI
  albo wersji API i wdrożeń Azure OpenAI istniejącego konta — bez usuwania konta i ponownego
  wpisywania klucza. Adres zasobu Azure się nie zmienia (inny zasób = nowe konto). Reguły i
  komunikaty: `logic/enterprise.ts` (te same co w rdzeniu); zapis: `accounts.setEnterprise`.
-->
<script lang="ts">
  import { untrack } from 'svelte';
  import { Button, IconButton, TextField } from '@alfa/ui-kit';
  import Plus from '@lucide/svelte/icons/plus';
  import X from '@lucide/svelte/icons/x';
  import { errorText } from '../../../api/command-error';
  import type { Account } from '../../../api/types-hub';
  import {
    AZURE_DEFAULT_API_VERSION,
    AZURE_OPENAI,
    MAX_DEPLOYMENTS,
    checkEnterprise,
    formToSettings,
    settingsToForm,
  } from '../../../logic/enterprise';
  import { useApp } from '../../../state/context';

  interface Props {
    account: Account;
    /** Konto po zapisie (lista kont odświeża się sama). */
    onsaved: (account: Account) => unknown;
    onclose: () => void;
  }

  let { account, onsaved, onclose }: Props = $props();
  const app = useApp();
  const { t } = app.i18n;
  const uid = $props.id();

  /** Formularz startuje z ustawień zapisanych przy koncie. */
  let form = $state(untrack(() => settingsToForm(account.provider_id, account.enterprise)));
  let saving = $state(false);
  let error = $state('');
  let saved = $state(false);

  const azure = $derived(account.provider_id === AZURE_OPENAI);
  const check = $derived(
    checkEnterprise(account.provider_id, account.base_url ?? null, formToSettings(form), true),
  );

  function addDeployment() {
    form.deployments = [...form.deployments, { name: '', model: '' }];
  }

  function removeDeployment(i: number) {
    form.deployments = form.deployments.filter((_, j) => j !== i);
  }

  async function save() {
    if (!check.ok || saving) return;
    saving = true;
    error = '';
    saved = false;
    try {
      const updated = await app.client.accounts.setEnterprise(account.id, check.value);
      form = settingsToForm(updated.provider_id, updated.enterprise);
      saved = true;
      await onsaved(updated);
    } catch (reason) {
      error = errorText(reason);
    } finally {
      saving = false;
    }
  }
</script>

<section class="ent" aria-labelledby="{uid}-title" data-testid="account-enterprise">
  <h4 id="{uid}-title">{t('ent.editTitle', { label: account.label })}</h4>
  {#if azure}
    <p class="note">
      {account.base_url ? t('ent.endpointFixed', { url: account.base_url }) : t('ent.endpointKept')}
    </p>
    <TextField
      label={t('ent.apiVersion')}
      bind:value={form.apiVersion}
      placeholder={AZURE_DEFAULT_API_VERSION}
      hint={t('ent.apiVersionHint', { version: AZURE_DEFAULT_API_VERSION })}
      spellcheck={false}
    />
    <fieldset>
      <legend>{t('ent.deployments')}</legend>
      <p class="note">{t('ent.deploymentsHint')}</p>
      {#each form.deployments as d, i (i)}
        <div class="row">
          <TextField
            aria-label={t('ent.deploymentName')}
            placeholder={t('ent.deploymentName')}
            bind:value={d.name}
            spellcheck={false}
          />
          <TextField
            aria-label={t('ent.deploymentModel')}
            placeholder={t('ent.deploymentModel')}
            bind:value={d.model}
            spellcheck={false}
          />
          <IconButton
            label={t('ent.deploymentRemove', { name: d.name || '…' })}
            onclick={() => removeDeployment(i)}><X size={14} /></IconButton
          >
        </div>
      {/each}
      <div class="actions">
        <Button
          size="sm"
          variant="secondary"
          disabled={form.deployments.length >= MAX_DEPLOYMENTS}
          onclick={addDeployment}
        >
          {#snippet icon()}<Plus size={14} strokeWidth={1.5} />{/snippet}
          {t('ent.deploymentAdd')}
        </Button>
      </div>
    </fieldset>
  {:else}
    <p class="note">{t('ent.editOpenai')}</p>
    <TextField
      label={t('ent.organization')}
      bind:value={form.organization}
      placeholder="org-…"
      hint={t('ent.organizationHint')}
      spellcheck={false}
    />
    <TextField
      label={t('ent.project')}
      bind:value={form.project}
      placeholder="proj_…"
      hint={t('ent.projectHint')}
      spellcheck={false}
    />
  {/if}
  {#if !check.ok}
    <p class="fail" role="alert">
      {t(`ent.err.${check.problem}`, {
        subject: check.subject,
        version: AZURE_DEFAULT_API_VERSION,
      })}
    </p>
  {:else if error}
    <p class="fail" role="alert">{error}</p>
  {:else if saved}
    <p class="ok" role="status">{t('ent.saved')}</p>
  {/if}
  <div class="actions">
    <Button size="sm" variant="primary" loading={saving} disabled={!check.ok} onclick={save}
      >{t('ent.save')}</Button
    >
    <Button size="sm" variant="ghost" onclick={onclose}>{t('common.close')}</Button>
  </div>
</section>

<style>
  .ent {
    display: flex;
    flex-direction: column;
    gap: var(--alfa-space-2);
  }
  h4 {
    margin: 0;
    font-size: var(--alfa-font-size-sm);
    font-weight: var(--alfa-weight-semibold);
  }
  fieldset {
    display: flex;
    flex-direction: column;
    gap: var(--alfa-space-2);
    margin: 0;
    padding: 0;
    border: none;
  }
  legend {
    padding: 0;
    font-size: var(--alfa-font-size-sm);
    font-weight: var(--alfa-weight-semibold);
  }
  .row {
    display: grid;
    grid-template-columns: 1fr 1fr auto;
    align-items: center;
    gap: var(--alfa-space-2);
  }
  .note {
    margin: 0;
    color: var(--alfa-color-text-muted);
    font-size: var(--alfa-font-size-sm);
  }
  .actions {
    display: flex;
    flex-wrap: wrap;
    gap: var(--alfa-space-1);
  }
  .fail {
    margin: 0;
    color: var(--alfa-color-error);
    font-size: var(--alfa-font-size-sm);
  }
  .ok {
    margin: 0;
    color: var(--alfa-color-success);
    font-size: var(--alfa-font-size-sm);
  }
</style>
