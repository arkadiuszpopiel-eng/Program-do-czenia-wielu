<!--
  Wprowadzenie (PLAN §14.5, makieta 14): mikrofon → profil głosu → pomiar sprzętu → konta i klucze
  („dodaj teraz" / „pomiń — dodam później") + pobranie modelu lokalnego → poziomy autonomii (start
  L3) → korpus (opcjonalnie)
  → import `.alfa` (opcjonalnie). Krok „mosty CLI" pojawi się od fali 4.
-->
<script lang="ts">
  import { untrack } from 'svelte';
  import {
    Avatar,
    Button,
    LevelMeter,
    SegmentedControl,
    Select,
    Stepper,
    agentIds,
  } from '@alfa/ui-kit';
  import type { AutonomyLevel } from '../../api/types';
  import type { AudioDevice, DeviceProfile, InspectResult } from '../../api/types-hub';
  import { useApp } from '../../state/context';
  import AddProviderWizard from '../settings/pages/AddProviderWizard.svelte';
  import BridgesStep from './BridgesStep.svelte';
  import LocalModelCard from './LocalModelCard.svelte';

  const app = useApp();
  const { t } = app.i18n;
  const STEPS = [
    'mic',
    'voice',
    'hardware',
    'keys',
    'autonomy',
    'corpus',
    'bridges',
    'import',
  ] as const;
  const LEVELS: readonly AutonomyLevel[] = ['L0', 'L1', 'L2', 'L3', 'L4'];

  let step = $state(untrack(() => app.onboardingStep));
  let devices = $state<readonly AudioDevice[]>([]);
  let device = $state('');
  let heard = $state(false);
  let micFailed = $state(false);
  let profile = $state<DeviceProfile | null>(null);
  let measuring = $state(false);
  let measured = false;
  let voice = $state<string>('B');
  let wizard = $state(false);
  let added = $state(0);
  let level = $state<string>('L3');
  let imported = $state<Extract<InspectResult, { status: 'inspected' }> | null>(null);
  let importDone = $state(false);

  const current = $derived(STEPS[step] ?? 'mic');

  $effect(() => {
    void app.client.voice.devices().then((list) => {
      devices = list;
      device = list.find((d) => d.default)?.id ?? list[0]?.id ?? '';
    });
    void app.client.device.profile().then((p) => {
      profile = p;
      voice = p.recommendation.voice_profile;
    });
  });

  // Test mikrofonu działa tylko na pierwszym kroku.
  $effect(() => {
    if (current !== 'mic') return;
    app.client.voice
      .startMicTest(device || null)
      .then(() => (micFailed = false))
      .catch(() => (micFailed = true));
    return () => void app.client.voice.stopMicTest().catch(() => undefined);
  });

  $effect(() => {
    if (app.micLevel > 0.2) heard = true;
  });

  $effect(() => {
    if (current !== 'hardware' || measured) return;
    measured = true;
    measuring = true;
    void app.client.device.measure().then((p) => {
      profile = p;
      measuring = false;
    });
  });

  function next() {
    if (step < STEPS.length - 1) step++;
    else void finish();
  }

  async function finish() {
    if (level !== 'L3') {
      // Podniesienie potwierdza tylko okno Brokera — odmowa nie blokuje końca wprowadzenia.
      try {
        await app.client.permissions.requestLevel(level as AutonomyLevel, null);
      } catch (error) {
        app.toasts.show({
          kind: 'warning',
          message: error instanceof Error ? error.message : String(error),
        });
      }
    }
    await app.client.app.completeOnboarding();
    app.system = await app.client.system.status();
    app.view = 'chat';
    if (!app.activeId) await app.newSession();
  }

  async function chooseImport() {
    const res = await app.client.transfer.inspect(null, null);
    if (res.status === 'inspected') imported = res;
  }

  async function runImport() {
    if (!imported) return;
    await app.client.transfer.importPackage({
      handle: imported.handle,
      mode: 'merge',
      resolutions: {},
      password: null,
    });
    importDone = true;
    app.sessions.list = [...(await app.client.sessions.list())];
  }
</script>

<main class="onboarding" aria-labelledby="ob-title">
  <div class="card">
    <header class="head">
      <div class="cast" aria-hidden="true">
        {#each agentIds as id (id)}<Avatar agent={id} size={28} />{/each}
      </div>
      <h1 id="ob-title">{t('ob.title')}</h1>
      <p class="muted">{t('ob.stepOf', { n: step + 1, total: STEPS.length })}</p>
      <Stepper
        steps={STEPS.map((id) => ({ id, label: t(`ob.step.${id}`) }))}
        current={step}
        label={t('ob.steps')}
        onselect={(i) => (step = i)}
      />
    </header>

    <section class="body" aria-live="polite">
      {#if current === 'mic'}
        <h2>{t('ob.mic.title')}</h2>
        <p class="muted">{t('ob.mic.desc')}</p>
        {#if app.system?.mic === 'denied'}
          <p class="warn">{t('banner.micDenied')}</p>
          <Button
            variant="secondary"
            onclick={() => void app.client.app.openSystemSettings('ms-settings:privacy-microphone')}
            >{t('banner.micSettings')}</Button
          >
        {:else if devices.length === 0}
          <p class="warn">{t('banner.micMissing')}</p>
        {:else}
          <label class="field">
            <span>{t('ob.mic.device')}</span>
            <Select
              bind:value={device}
              options={devices.map((d) => ({ value: d.id, label: d.name }))}
              label={t('ob.mic.device')}
            />
          </label>
          <LevelMeter level={app.micLevel} label={t('ob.mic.level')} />
          {#if micFailed}
            <p class="warn">{t('ob.mic.failed')}</p>
          {:else}
            <p class:ok={heard} class="muted">{heard ? t('ob.mic.ok') : t('ob.mic.silent')}</p>
          {/if}
        {/if}
      {:else if current === 'voice'}
        <h2>{t('ob.voice.title')}</h2>
        <p class="muted">{t('ob.voice.desc')}</p>
        <SegmentedControl
          label={t('ob.voice.title')}
          variant="cards"
          bind:value={voice}
          options={(['A', 'B', 'C', 'D'] as const).map((v) => ({
            value: v,
            label: `${t(`ob.voice.${v}`)}${profile?.recommendation.voice_profile === v ? ` · ${t('ob.voice.recommended')}` : ''}`,
          }))}
        />
      {:else if current === 'hardware'}
        <h2>{t('ob.hw.title')}</h2>
        <p class="muted">{t('ob.hw.desc')}</p>
        {#if measuring || !profile}
          <p role="status">{t('ob.hw.measuring')}</p>
        {:else}
          <dl class="grid">
            <dt>{t('dev.cpu')}</dt>
            <dd>{profile.machine.cpu.model}</dd>
            <dt>{t('dev.ram')}</dt>
            <dd>{app.i18n.bytes(profile.machine.ram_mb * 1024 * 1024)}</dd>
            <dt>{t('dev.gpu')}</dt>
            <dd>
              {profile.machine.gpus.map((g) => `${g.vendor} ${g.model}`).join(', ') ||
                t('common.none')}
            </dd>
            <dt>{t('dev.hwClass')}</dt>
            <dd>{t(`dev.hw.${profile.recommendation.hw_class}`)}</dd>
            <dt>{t('dev.llm')}</dt>
            <dd>{profile.recommendation.llm_backend}</dd>
          </dl>
          <ul class="list">
            {#each profile.recommendation.tradeoffs as item (item.pl)}<li>
                {app.i18n.text(item)}
              </li>{/each}
          </ul>
        {/if}
      {:else if current === 'keys'}
        <h2>{t('ob.keys.title')}</h2>
        <p class="muted">{t('ob.keys.desc')}</p>
        {#if wizard}
          <AddProviderWizard
            onfinish={(account) => {
              wizard = false;
              if (account) added++;
            }}
          />
        {:else}
          {#if added > 0}<p class="ok" role="status">{t('ob.keys.added', { n: added })}</p>{/if}
          <div class="choices">
            <Button variant="primary" onclick={() => (wizard = true)}>{t('ob.keys.addNow')}</Button>
            <Button variant="secondary" onclick={next}>{t('ob.keys.skip')}</Button>
          </div>
          <LocalModelCard />
        {/if}
      {:else if current === 'autonomy'}
        <h2>{t('ob.autonomy.title')}</h2>
        <p class="muted">{t('ob.autonomy.desc')}</p>
        <SegmentedControl
          label={t('perm.title')}
          variant="cards"
          bind:value={level}
          options={LEVELS.map((l) => ({
            value: l,
            label: `${l} · ${t(`perm.${l}.name`)}`,
            description: t(`perm.${l}.desc`),
          }))}
        />
        <p class="muted small">{t('perm.kept')}</p>
      {:else if current === 'corpus'}
        <h2>{t('ob.corpus.title')}</h2>
        <p class="muted">{t('ob.corpus.desc')}</p>
      {:else if current === 'bridges'}
        <BridgesStep />
      {:else}
        <h2>{t('ob.import.title')}</h2>
        <p class="muted">{t('ob.import.desc')}</p>
        {#if imported}
          <p class="muted small">
            {t('tr.manifest', {
              machine: imported.manifest.source_machine,
              date: app.i18n.dateTime(imported.manifest.created_at),
              schema: imported.manifest.schema_version,
            })}
          </p>
          <ul class="list">
            {#each imported.items as item (item.key)}<li>
                {item.label} — {t(`tr.diff.${item.diff}`)}
              </li>{/each}
          </ul>
          {#if importDone}
            <p class="ok" role="status">{t('tr.imported', { n: imported.items.length })}</p>
          {:else}
            <Button variant="primary" onclick={runImport}
              >{t('tr.importButton')} ({t('tr.mode.merge')})</Button
            >
          {/if}
        {:else}
          <Button variant="secondary" onclick={chooseImport}>{t('tr.choose')}</Button>
        {/if}
      {/if}
    </section>

    <footer class="foot">
      <Button variant="ghost" disabled={step === 0} onclick={() => (step = Math.max(0, step - 1))}
        >{t('common.back')}</Button
      >
      <span class="spacer"></span>
      {#if current === 'corpus' || current === 'import'}
        <Button variant="secondary" onclick={next}
          >{current === 'corpus' ? t('ob.later') : t('ob.skip')}</Button
        >
      {/if}
      {#if current !== 'keys' || wizard === false}
        <Button variant="primary" onclick={next}
          >{step === STEPS.length - 1 ? t('ob.finish') : t('common.next')}</Button
        >
      {/if}
    </footer>
  </div>
</main>

<style>
  .onboarding {
    display: flex;
    align-items: flex-start;
    justify-content: center;
    height: 100%;
    overflow: auto;
    padding: var(--alfa-space-8) var(--alfa-space-4);
    background: var(--alfa-color-bg);
  }
  .card {
    display: flex;
    flex-direction: column;
    gap: var(--alfa-space-4);
    width: min(720px, 100%);
    padding: var(--alfa-space-6);
    border: 1px solid var(--alfa-color-border);
    border-radius: var(--alfa-radius-overlay);
    background: var(--alfa-color-surface);
    box-shadow: var(--alfa-shadow-2);
  }
  .head {
    display: flex;
    flex-direction: column;
    gap: var(--alfa-space-2);
  }
  .cast {
    display: flex;
    gap: var(--alfa-space-1);
  }
  h1 {
    font-size: var(--alfa-font-size-2xl);
  }
  h2 {
    font-size: var(--alfa-font-size-xl);
  }
  .body {
    display: flex;
    flex-direction: column;
    gap: var(--alfa-space-3);
    min-height: 260px;
  }
  .muted {
    color: var(--alfa-color-text-muted);
  }
  .small {
    font-size: var(--alfa-font-size-sm);
  }
  .ok {
    color: var(--alfa-color-success);
    font-weight: var(--alfa-weight-semibold);
  }
  .warn {
    color: var(--alfa-color-warning);
    font-weight: var(--alfa-weight-semibold);
  }
  .field {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: var(--alfa-space-1);
    font-size: var(--alfa-font-size-sm);
    font-weight: var(--alfa-weight-semibold);
  }
  .grid {
    display: grid;
    grid-template-columns: max-content 1fr;
    gap: var(--alfa-space-1) var(--alfa-space-4);
    margin: 0;
  }
  dt {
    color: var(--alfa-color-text-muted);
  }
  dd {
    margin: 0;
  }
  .list {
    margin: 0;
    padding-left: var(--alfa-space-4);
    font-size: var(--alfa-font-size-sm);
  }
  .choices {
    display: flex;
    flex-wrap: wrap;
    gap: var(--alfa-space-2);
  }
  .foot {
    display: flex;
    align-items: center;
    gap: var(--alfa-space-2);
    padding-top: var(--alfa-space-3);
    border-top: 1px solid var(--alfa-color-border);
  }
  .spacer {
    flex: 1;
  }
</style>
