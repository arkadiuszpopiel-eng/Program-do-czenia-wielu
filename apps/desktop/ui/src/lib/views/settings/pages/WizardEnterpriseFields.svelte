<!--
  Kreator „Dodaj dostawcę” → krok „Klucz”: konto firmowe (C2.12). OpenAI — opcjonalna organizacja
  i projekt (nagłówki `OpenAI-Organization`, `OpenAI-Project`); Azure OpenAI — wersja API i wdrożenia
  modeli w zasobie. Reguły i komunikaty: `logic/enterprise.ts` (te same co w rdzeniu). Adres
  wdrożenia wklejony z portalu w pole adresu da się jednym przyciskiem rozdzielić na zasób,
  wdrożenie i wersję API.
-->
<script lang="ts">
  import { Button, Checkbox, IconButton, TextField } from '@alfa/ui-kit';
  import Plus from '@lucide/svelte/icons/plus';
  import X from '@lucide/svelte/icons/x';
  import type { ProviderInfo } from '../../../api/types-hub';
  import {
    AZURE_DEFAULT_API_VERSION,
    AZURE_OPENAI,
    MAX_DEPLOYMENTS,
    OPENAI,
    checkEnterprise,
    formToSettings,
    parseAzureTarget,
    type EnterpriseForm,
  } from '../../../logic/enterprise';
  import { useApp } from '../../../state/context';

  interface Props {
    provider: ProviderInfo;
    form: EnterpriseForm;
    baseUrl: string;
  }

  let { provider, form = $bindable(), baseUrl = $bindable() }: Props = $props();
  const app = useApp();
  const { t } = app.i18n;
  const uid = $props.id();

  let open = $state(false);
  /** Pola widać, gdy użytkownik je włączył albo już coś w nich jest (powrót do kroku klucza). */
  const company = $derived(open || form.organization.trim() !== '' || form.project.trim() !== '');
  const azure = $derived(provider.id === AZURE_OPENAI);
  const check = $derived(
    checkEnterprise(provider.id, baseUrl.trim() || null, formToSettings(form)),
  );
  /** Błąd pokazujemy dopiero, gdy jest co oceniać (nie na pustym formularzu). */
  const touched = $derived(
    baseUrl.trim() !== '' ||
      [form.organization, form.project, form.apiVersion].some((v) => v.trim()) ||
      form.deployments.some((d) => d.name.trim() || d.model.trim()),
  );
  const target = $derived(azure ? parseAzureTarget(baseUrl) : null);

  function toggleCompany(on: boolean) {
    open = on;
    // Wyłączone pola nie mogą po cichu trafić do konta.
    if (!on) form = { ...form, organization: '', project: '' };
  }

  function addDeployment() {
    form.deployments = [...form.deployments, { name: '', model: '' }];
  }

  function removeDeployment(i: number) {
    form.deployments = form.deployments.filter((_, j) => j !== i);
  }

  /** Adres wdrożenia z portalu → adres zasobu + wiersz wdrożenia + wersja API. */
  function splitTarget() {
    if (!target) return;
    baseUrl = target.endpoint;
    if (target.apiVersion && !form.apiVersion.trim()) form.apiVersion = target.apiVersion;
    const deployment = target.deployment;
    if (deployment && !form.deployments.some((d) => d.name.trim() === deployment)) {
      const rows = form.deployments.filter((d) => d.name.trim() || d.model.trim());
      form.deployments = [...rows, { name: deployment, model: '' }];
    }
  }
</script>

{#if provider.id === OPENAI}
  <section class="ent" aria-labelledby="{uid}-title">
    <h5 id="{uid}-title">{t('ent.openaiTitle')}</h5>
    <Checkbox label={t('ent.openaiToggle')} checked={company} onchange={toggleCompany} />
    {#if company}
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
  </section>
{:else if azure}
  <section class="ent" aria-labelledby="{uid}-title">
    <h5 id="{uid}-title">{t('ent.azureTitle')}</h5>
    <p class="note">{t('ent.azureWhere')}</p>
    {#if target}
      <div class="target" role="status">
        <span>{t('ent.target')}</span>
        <Button size="sm" variant="secondary" onclick={splitTarget}>{t('ent.targetUse')}</Button>
      </div>
    {/if}
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
  </section>
{/if}
{#if !check.ok && touched}
  <p class="fail" role="alert">
    {t(`ent.err.${check.problem}`, {
      subject: check.subject,
      version: AZURE_DEFAULT_API_VERSION,
    })}
  </p>
{/if}

<style>
  .ent {
    display: flex;
    flex-direction: column;
    gap: var(--alfa-space-2);
    padding: var(--alfa-space-3) var(--alfa-space-4);
    border: 1px solid var(--alfa-color-border);
    border-radius: var(--alfa-radius-control);
  }
  h5 {
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
  .target {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--alfa-space-2);
    color: var(--alfa-color-warning);
    font-size: var(--alfa-font-size-sm);
  }
  .note {
    margin: 0;
    color: var(--alfa-color-text-muted);
    font-size: var(--alfa-font-size-sm);
  }
  .actions {
    display: flex;
    justify-content: flex-start;
  }
  .fail {
    margin: 0;
    color: var(--alfa-color-error);
    font-size: var(--alfa-font-size-sm);
  }
</style>
