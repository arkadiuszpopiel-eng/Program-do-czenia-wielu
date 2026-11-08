<!--
  Kreator „Dodaj dostawcę" (PLAN §5.6, makieta 12): dostawca → klucz → test → modele → przypisanie → limit.
  Klucz trafia jednorazowo do rdzenia (Credential Manager); pole jest czyszczone po zapisaniu konta.
  Gdy rdzeń odmówi (np. brak adresu endpointu), komunikat jest widoczny, a klucz zostaje w polu.
  Po zapisaniu konta nie wracamy do kroku klucza (drugie „Dalej" utworzyłoby drugie konto) —
  „Popraw klucz" usuwa świeżo dodane konto i dopiero wtedy wraca do pola klucza.
-->
<script lang="ts">
  import {
    Button,
    Checkbox,
    Stepper,
    Switch,
    TextField,
    agentIds,
    agents,
    type AgentId,
  } from '@alfa/ui-kit';
  import { errorText } from '../../../api/command-error';
  import { cleanKey } from '../../../logic/api-key';
  import {
    checkEnterprise,
    emptyEnterpriseForm,
    formToSettings,
    type EnterpriseForm,
  } from '../../../logic/enterprise';
  import type { Account, ProviderInfo, TestReport } from '../../../api/types-hub';
  import { attempt, showError } from '../../../state/attempt';
  import { useApp } from '../../../state/context';
  import ProviderPicker from './ProviderPicker.svelte';
  import WizardEnterpriseFields from './WizardEnterpriseFields.svelte';
  import WizardKeyStep from './WizardKeyStep.svelte';
  import WizardTestResult from './WizardTestResult.svelte';

  interface Props {
    onfinish: (account: Account | null) => void;
  }

  let { onfinish }: Props = $props();
  const app = useApp();
  const { t } = app.i18n;
  const STEPS = ['provider', 'key', 'test', 'models', 'assign', 'limit'] as const;
  // Klasy zadań znane rdzeniowi (crates/app-core/src/commands/accounts.rs, TASKS) — nieznaną
  // rdzeń po cichu pomija, więc oferujemy tylko te.
  const TASKS = ['chat', 'code', 'planning', 'summarize'] as const;

  let step = $state(0);
  let provider = $state<ProviderInfo | null>(null);
  let label = $state('');
  let secret = $state('');
  let baseUrl = $state('');
  /** Konto firmowe: organizacja/projekt OpenAI albo wdrożenia Azure OpenAI. */
  let enterprise = $state<EnterpriseForm>(emptyEnterpriseForm());
  const enterpriseCheck = $derived(
    checkEnterprise(provider?.id ?? '', baseUrl.trim() || null, formToSettings(enterprise)),
  );
  let account = $state<Account | null>(null);
  let report = $state<TestReport | null>(null);
  let testing = $state(false);
  let saving = $state(false);
  let finishing = $state(false);
  let keyError = $state('');
  let tasks = $state<string[]>(['chat']);
  let assigned = $state<AgentId[]>([...agentIds]);
  let stt = $state(false);
  let tts = $state(false);
  let limitOn = $state(true);
  let limitZl = $state(100);

  function choose(p: ProviderInfo) {
    // Zmiana dostawcy czyści wszystko, co dotyczyło poprzedniego. Adres endpointu wpisany dla
    // innego dostawcy wysłałby ten klucz na cudzy serwer.
    if (provider?.id !== p.id) {
      baseUrl = '';
      secret = '';
      enterprise = emptyEnterpriseForm(p.id);
    }
    provider = p;
    label = p.display_name;
    keyError = '';
    step = 1;
  }

  async function saveKey() {
    // Do rdzenia idzie sam klucz — bez cudzysłowów, `NAZWA=` i znaków nowej linii ze schowka.
    const key = cleanKey(secret);
    if (!provider || !key || saving) return;
    saving = true;
    keyError = '';
    try {
      account = await app.client.accounts.add({
        provider_id: provider.id,
        label: label.trim() || provider.display_name,
        secret: key,
        // Adres tylko wtedy, gdy kreator o niego pytał (dostawca bez adresu w katalogu).
        base_url: provider.needs_base_url ? baseUrl.trim() || null : null,
        // Rdzeń sprawdza je jeszcze raz; tu idą już oczyszczone (bez pustych pól).
        ...(enterpriseCheck.ok ? { enterprise: enterpriseCheck.value } : {}),
      });
    } catch (error) {
      keyError = errorText(error);
      return;
    } finally {
      saving = false;
    }
    secret = '';
    step = 2;
    await runTest();
  }

  async function runTest() {
    if (!account) return;
    testing = true;
    try {
      report = await app.client.accounts.test(account.id);
    } catch (error) {
      report = {
        ok: false,
        latency_ms: null,
        models: [],
        error: errorText(error),
        reason: 'network',
      };
    } finally {
      testing = false;
    }
  }

  /** „Popraw klucz" po nieudanym teście: konto z błędnym kluczem usuwamy, zanim wrócimy do pola. */
  async function editKey() {
    const created = account;
    if (!created || saving) return;
    saving = true;
    const removed = await attempt(app.toasts, () => app.client.accounts.remove(created.id));
    saving = false;
    if (!removed) return;
    account = null;
    report = null;
    step = 1;
  }

  /** Model wpisany ręcznie (klucz bez prawa do listy modeli) — bez niego Alfa nie ma czego wywołać. */
  async function setModel(model: string) {
    const created = account;
    if (!created || !report) return;
    try {
      const updated = await app.client.accounts.setModel(created.id, model);
      account = updated;
      report = { ...report, models: updated.models };
    } catch (error) {
      showError(app.toasts, error);
    }
  }

  /** Stepper: przed zapisem konta tylko kroki 1–2, po zapisie tylko kroki od testu wzwyż. */
  function select(index: number) {
    if (account ? index >= 2 : index < 2) step = index;
  }

  async function finish() {
    const created = account;
    if (!created || finishing) return;
    finishing = true;
    try {
      await app.client.accounts.assign(created.id, {
        task_classes: tasks,
        // Wszystkie agentki = bez przypięcia: przypięte konto idzie przed doborem trudności, więc
        // każde krótkie pytanie trafiało do pierwszego (najdroższego) modelu konta (przegląd 2026-10-22).
        agents: agentIds.every((id) => assigned.includes(id)) ? [] : assigned,
        voice_stt: stt,
        voice_tts: tts,
      });
      await app.client.accounts.setLimit(created.id, limitOn, {
        minor: Math.round(limitZl * 100),
        currency: 'PLN',
      });
    } catch (error) {
      showError(app.toasts, error);
      return;
    } finally {
      finishing = false;
    }
    app.toasts.show({ kind: 'success', message: t('wiz.added', { label: created.label }) });
    onfinish(created);
  }

  function toggleIn<T>(list: T[], item: T, on: boolean): T[] {
    return on ? [...list, item] : list.filter((x) => x !== item);
  }
</script>

<section class="wizard" aria-labelledby="wiz-title">
  <h3 id="wiz-title">{t('wiz.title')}</h3>
  <Stepper
    steps={STEPS.map((id) => ({ id, label: t(`wiz.step.${id}`) }))}
    current={step}
    label={t('wiz.stepOf', {
      n: step + 1,
      total: STEPS.length,
      name: t(`wiz.step.${STEPS[step] ?? 'provider'}`),
    })}
    onselect={select}
  />
  <div class="body">
    {#if step === 0}
      <ProviderPicker onchoose={choose} />
    {:else if step === 1 && provider}
      <WizardKeyStep
        {provider}
        bind:secret
        bind:label
        bind:baseUrl
        {saving}
        error={keyError}
        onsubmit={saveKey}
        blocked={!enterpriseCheck.ok}
      >
        {#snippet extra()}
          {#if provider}
            <WizardEnterpriseFields {provider} bind:form={enterprise} bind:baseUrl />
          {/if}
        {/snippet}
      </WizardKeyStep>
    {:else if step === 2}
      <WizardTestResult
        {report}
        {testing}
        providerId={provider?.id ?? ''}
        providerName={provider?.display_name ?? ''}
        busy={saving}
        onnext={() => (step = 3)}
        onretry={runTest}
        onfix={editKey}
        onkeep={() => onfinish(account)}
        onmodel={setModel}
      />
    {:else if step === 3 && report}
      <p>{t('wiz.modelsFound', { n: report.models.length })}</p>
      <ul class="models">
        {#each report.models as m (m.id)}
          <li>
            <strong>{m.name}</strong>
            <span class="p-meta"
              >{m.id}{m.context_tokens ? ` · ${app.i18n.int(m.context_tokens)}` : ''}</span
            >
          </li>
        {/each}
      </ul>
      <div class="actions">
        <Button variant="primary" onclick={() => (step = 4)}>{t('common.next')}</Button>
      </div>
    {:else if step === 4}
      <fieldset>
        <legend>{t('wiz.tasks')}</legend>
        {#each TASKS as task (task)}
          <Checkbox
            label={t(`wiz.task.${task}`)}
            checked={tasks.includes(task)}
            onchange={(on) => (tasks = toggleIn(tasks, task, on))}
          />
        {/each}
      </fieldset>
      <fieldset>
        <legend>{t('wiz.agents')}</legend>
        {#each agentIds as id (id)}
          <Checkbox
            label={agents[id].name}
            checked={assigned.includes(id)}
            onchange={(on) => (assigned = toggleIn(assigned, id, on))}
          />
        {/each}
      </fieldset>
      <fieldset>
        <legend>{t('wiz.voice')}</legend>
        <Checkbox label={t('wiz.stt')} bind:checked={stt} />
        <Checkbox label={t('wiz.tts')} bind:checked={tts} />
      </fieldset>
      <div class="actions">
        <Button variant="primary" onclick={() => (step = 5)}>{t('common.next')}</Button>
      </div>
    {:else if step === 5}
      <div class="limit">
        <span id="wiz-limit">{t('wiz.limitToggle')}</span>
        <Switch bind:checked={limitOn} labelledby="wiz-limit" />
      </div>
      {#if limitOn}<TextField
          label={t('wiz.limitAmount')}
          type="number"
          min="0"
          step="10"
          bind:value={limitZl}
        />{/if}
      <div class="actions">
        <Button
          variant="primary"
          disabled={finishing || (limitOn && !(Number(limitZl) > 0))}
          loading={finishing}
          onclick={finish}>{t('wiz.finish')}</Button
        >
      </div>
    {/if}
  </div>
  <div class="foot">
    <!-- Konto zapisane w kroku 2 istnieje — „Pomiń" przekazuje je dalej, a nie `null`. -->
    <Button variant="ghost" onclick={() => onfinish(account)}
      >{account ? t('wiz.finishLater') : t('wiz.skip')}</Button
    >
  </div>
</section>

<style>
  .wizard {
    display: flex;
    flex-direction: column;
    gap: var(--alfa-space-3);
    padding: var(--alfa-space-4);
    border: 1px solid var(--alfa-color-border-strong);
    border-radius: var(--alfa-radius-card);
    background: var(--alfa-color-surface);
  }
  h3 {
    font-size: var(--alfa-font-size-lg);
  }
  .body {
    display: flex;
    flex-direction: column;
    gap: var(--alfa-space-3);
  }
  .p-meta {
    color: var(--alfa-color-text-muted);
    font-size: var(--alfa-font-size-xs);
  }
  .models {
    margin: 0;
    padding-left: var(--alfa-space-4);
    font-size: var(--alfa-font-size-sm);
  }
  fieldset {
    display: flex;
    flex-wrap: wrap;
    gap: var(--alfa-space-1) var(--alfa-space-4);
    margin: 0;
    padding: var(--alfa-space-2) var(--alfa-space-3);
    border: 1px solid var(--alfa-color-border);
    border-radius: var(--alfa-radius-control);
  }
  legend {
    padding: 0 var(--alfa-space-1);
    font-size: var(--alfa-font-size-sm);
    font-weight: var(--alfa-weight-semibold);
  }
  .limit {
    display: flex;
    align-items: center;
    justify-content: space-between;
    font-size: var(--alfa-font-size-sm);
  }
  .actions,
  .foot {
    display: flex;
    justify-content: flex-end;
    gap: var(--alfa-space-2);
  }
  .foot {
    justify-content: flex-start;
    border-top: 1px solid var(--alfa-color-border);
    padding-top: var(--alfa-space-2);
  }
</style>
