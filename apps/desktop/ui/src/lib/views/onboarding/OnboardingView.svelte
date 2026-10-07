<!--
  Wprowadzenie (PLAN §14.5, makieta 14): mikrofon → profil głosu → pomiar sprzętu → konta i klucze
  („dodaj teraz" / „pomiń — dodam później") + pobranie modelu lokalnego → poziomy autonomii (start
  L3) → korpus (opcjonalnie)
  → import `.alfa` (opcjonalnie). Krok „mosty CLI" pojawi się od fali 4. Kroki mikrofonu i importu:
  `MicStep`, `ImportStep`. Każdy błąd rdzenia jest widoczny (toast albo „Ponów"), nigdy cichy.
-->
<script lang="ts">
  import { untrack } from 'svelte';
  import { Avatar, Button, SegmentedControl, Stepper, agentIds } from '@alfa/ui-kit';
  import type { AutonomyLevel } from '../../api/types';
  import { errorText } from '../../api/command-error';
  import type { DeviceProfile } from '../../api/types-hub';
  import LoadFailed from '../../components/shell/LoadFailed.svelte';
  import { attempt } from '../../state/attempt';
  import { useApp } from '../../state/context';
  import AddProviderWizard from '../settings/pages/AddProviderWizard.svelte';
  import BridgesStep from './BridgesStep.svelte';
  import ImportStep from './ImportStep.svelte';
  import LocalModelCard from './LocalModelCard.svelte';
  import MicStep from './MicStep.svelte';

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
  let profile = $state<DeviceProfile | null>(null);
  let measuring = $state(false);
  let measured = false;
  let measureError = $state<string | null>(null);
  let voice = $state<string>('B');
  let wizard = $state(false);
  let added = $state(0);
  let level = $state<string>('L3');
  let finishing = $state(false);

  const current = $derived(STEPS[step] ?? 'mic');

  $effect(() => {
    // Tylko rekomendacja głosu; bez niej krok „głos" działa, a pomiar (z „Ponów") jest w kroku „sprzęt".
    app.client.device.profile().then(
      (p) => {
        profile = p;
        voice = p.recommendation.voice_profile;
      },
      () => undefined,
    );
  });

  /** Pomiar sprzętu: `measuring` zawsze wraca do `false`, a błąd daje „Ponów" (nie wieczne „Mierzę…"). */
  async function measure() {
    measuring = true;
    measureError = null;
    try {
      profile = await app.client.device.measure();
      measured = true;
    } catch (error) {
      measureError = errorText(error);
    } finally {
      measuring = false;
    }
  }

  $effect(() => {
    if (current !== 'hardware' || measured || untrack(() => measuring)) return;
    void measure();
  });

  function next() {
    if (step < STEPS.length - 1) step++;
    else void finish();
  }

  async function finish() {
    if (finishing) return;
    finishing = true;
    try {
      if (level !== 'L3') {
        // Podniesienie potwierdza tylko okno Brokera — odmowa nie blokuje końca wprowadzenia.
        try {
          await app.client.permissions.requestLevel(level as AutonomyLevel, null);
        } catch (error) {
          app.toasts.show({ kind: 'warning', message: errorText(error) });
        }
      }
      if (!(await attempt(app.toasts, () => app.client.app.completeOnboarding()))) return;
      await attempt(app.toasts, async () => {
        app.system = await app.client.system.status();
      });
      app.view = 'chat';
      if (!app.activeId) await attempt(app.toasts, () => app.newSession());
    } finally {
      finishing = false;
    }
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
        <MicStep />
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
        {#if measureError && !measuring}
          <LoadFailed error={measureError} onretry={() => void measure()} />
        {:else if measuring || !profile}
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
        <ImportStep />
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
        <Button variant="primary" loading={finishing} disabled={finishing} onclick={next}
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
