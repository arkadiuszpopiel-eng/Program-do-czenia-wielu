<!--
  Kreator „Dodaj dostawcę" (PLAN §5.6, makieta 12): dostawca → klucz → test → modele → przypisanie → limit.
  Klucz trafia jednorazowo do rdzenia (Credential Manager); pole jest czyszczone od razu po wysłaniu.
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
  import type { Account, ProviderInfo, TestReport } from '../../../api/types-hub';
  import { fuzzyRank } from '../../../logic/fuzzy';
  import { useApp } from '../../../state/context';

  interface Props {
    onfinish: (account: Account | null) => void;
  }

  let { onfinish }: Props = $props();
  const app = useApp();
  const { t } = app.i18n;
  const STEPS = ['provider', 'key', 'test', 'models', 'assign', 'limit'] as const;
  const TASKS = ['chat', 'code', 'background'] as const;

  let step = $state(0);
  let catalog = $state<readonly ProviderInfo[]>([]);
  let filter = $state('');
  let provider = $state<ProviderInfo | null>(null);
  let label = $state('');
  let secret = $state('');
  let baseUrl = $state('');
  let account = $state<Account | null>(null);
  let report = $state<TestReport | null>(null);
  let testing = $state(false);
  let tasks = $state<string[]>(['chat']);
  let assigned = $state<AgentId[]>([...agentIds]);
  let stt = $state(false);
  let tts = $state(false);
  let limitOn = $state(true);
  let limitZl = $state(100);

  $effect(() => {
    void app.client.accounts.catalog().then((c) => (catalog = c));
  });

  const shown = $derived(
    filter.trim()
      ? fuzzyRank(catalog, filter, (p) => ({ label: p.display_name, keywords: [p.id] })).map(
          (r) => r.item,
        )
      : catalog,
  );

  function choose(p: ProviderInfo) {
    provider = p;
    label = p.display_name;
    step = 1;
  }

  async function saveKey() {
    if (!provider || !secret.trim()) return;
    const input = {
      provider_id: provider.id,
      label: label.trim() || provider.display_name,
      secret,
      base_url: baseUrl.trim() || null,
    };
    secret = '';
    account = await app.client.accounts.add(input);
    step = 2;
    await runTest();
  }

  async function runTest() {
    if (!account) return;
    testing = true;
    report = await app.client.accounts.test(account.id);
    testing = false;
  }

  async function finish() {
    if (!account) return;
    await app.client.accounts.assign(account.id, {
      task_classes: tasks,
      agents: assigned,
      voice_stt: stt,
      voice_tts: tts,
    });
    await app.client.accounts.setLimit(account.id, limitOn, {
      minor: Math.round(limitZl * 100),
      currency: 'PLN',
    });
    app.toasts.show({ kind: 'success', message: t('wiz.added', { label: account.label }) });
    onfinish(account);
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
    onselect={(i) => (step = i < 2 ? i : step)}
  />
  <div class="body" aria-live="polite">
    {#if step === 0}
      <TextField label={t('wiz.filter')} type="search" bind:value={filter} />
      <ul class="providers">
        {#each shown as p (p.id)}
          <li>
            <button type="button" class="provider" onclick={() => choose(p)}>
              <span class="p-name">{p.display_name}</span>
              <span class="p-meta"
                >{t('hub.privacy', { tag: p.privacy_tag, jurisdiction: p.jurisdiction })} · {t(
                  `hub.compliance.${p.compliance_status}`,
                )}</span
              >
            </button>
          </li>
        {/each}
      </ul>
    {:else if step === 1 && provider}
      <TextField label={t('wiz.label')} bind:value={label} autocomplete="off" />
      <TextField
        label={t('wiz.key')}
        type="password"
        bind:value={secret}
        hint={t('wiz.keyHint')}
        autocomplete="off"
        spellcheck={false}
      />
      {#if provider.needs_base_url}
        <TextField
          label={t('wiz.baseUrl')}
          type="url"
          bind:value={baseUrl}
          placeholder="https://"
        />
      {/if}
      <div class="actions">
        <Button variant="primary" disabled={!secret.trim()} onclick={saveKey}
          >{t('common.next')}</Button
        >
      </div>
    {:else if step === 2}
      {#if testing}
        <p role="status">{t('wiz.testing')}</p>
      {:else if report?.ok}
        <p class="ok" role="status">
          {t('wiz.testOk', { latency: app.i18n.duration(report.latency_ms ?? 0) })}
        </p>
        <div class="actions">
          <Button variant="primary" onclick={() => (step = 3)}>{t('common.next')}</Button>
        </div>
      {:else if report}
        <p class="fail" role="alert">{t('wiz.testFail', { error: report.error ?? '' })}</p>
        <div class="actions">
          <Button variant="secondary" onclick={() => (step = 1)}>{t('common.back')}</Button>
          <Button variant="primary" onclick={runTest}>{t('common.retry')}</Button>
        </div>
      {/if}
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
        <Button variant="primary" onclick={finish}>{t('wiz.finish')}</Button>
      </div>
    {/if}
  </div>
  <div class="foot">
    <Button variant="ghost" onclick={() => onfinish(null)}>{t('wiz.skip')}</Button>
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
  .providers {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(220px, 1fr));
    gap: var(--alfa-space-2);
    max-height: 320px;
    margin: 0;
    padding: 0;
    overflow: auto;
    list-style: none;
  }
  .provider {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 2px;
    width: 100%;
    padding: var(--alfa-space-2) var(--alfa-space-3);
    border: 1px solid var(--alfa-color-border);
    border-radius: var(--alfa-radius-control);
    background: var(--alfa-color-bg);
    color: var(--alfa-color-text);
    text-align: left;
  }
  .provider:hover {
    border-color: var(--alfa-color-border-strong);
  }
  .p-name {
    font-weight: var(--alfa-weight-semibold);
    font-size: var(--alfa-font-size-sm);
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
  .ok {
    color: var(--alfa-color-success);
    font-weight: var(--alfa-weight-semibold);
  }
  .fail {
    color: var(--alfa-color-error);
  }
</style>
