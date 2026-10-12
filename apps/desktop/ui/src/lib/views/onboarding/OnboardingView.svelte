<!--
  Wprowadzenie (PLAN §14.5, makieta 14): mikrofon → profil głosu → pomiar sprzętu → konta i klucze
  („dodaj teraz" / „pomiń — dodam później") + pakiet modelu lokalnego → poziomy autonomii (start
  L3) → korpus (opcjonalnie) → mosty CLI (opcjonalnie) → import `.alfa` (opcjonalnie).
  Układ: boczna szyna z krokami (`OnboardingRail`) i panel kroku o stałej wysokości — stopka
  z przyciskami nie skacze między krokami, a dłuższa treść przewija się w panelu.
  Każdy błąd rdzenia jest widoczny (toast albo „Ponów"), nigdy cichy.
-->
<script lang="ts">
  import { untrack } from 'svelte';
  import { Button } from '@alfa/ui-kit';
  import type { AutonomyLevel } from '../../api/types';
  import { errorText } from '../../api/command-error';
  import type { DeviceProfile } from '../../api/types-hub';
  import { attempt } from '../../state/attempt';
  import { useApp } from '../../state/context';
  import AddProviderWizard from '../settings/pages/AddProviderWizard.svelte';
  import BridgesStep from './BridgesStep.svelte';
  import HardwareStep from './HardwareStep.svelte';
  import ImportStep from './ImportStep.svelte';
  import LocalBundleCard from './LocalBundleCard.svelte';
  import MicStep from './MicStep.svelte';
  import OnboardingRail from './OnboardingRail.svelte';
  import OptionCards from './OptionCards.svelte';
  import StepIcon from './StepIcon.svelte';

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
  const OPTIONAL: readonly string[] = ['corpus', 'bridges', 'import'];
  const LEVELS: readonly AutonomyLevel[] = ['L0', 'L1', 'L2', 'L3', 'L4'];
  const VOICES = ['A', 'B', 'C', 'D'] as const;

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
  /** Tytuł i opis kroku w nagłówku panelu (kroki z własnym komponentem mają te same klucze). */
  const heading = $derived(t(`ob.${current === 'hardware' ? 'hw' : current}.title`));
  const lead = $derived(t(`ob.${current === 'hardware' ? 'hw' : current}.desc`));

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

<main class="onboarding" data-surface="app" aria-labelledby="ob-title">
  <div class="shell">
    <OnboardingRail
      steps={STEPS.map((id) => ({
        id,
        label: t(`ob.step.${id}`),
        optional: OPTIONAL.includes(id),
      }))}
      current={step}
      title={t('ob.title')}
      tagline={t('ob.tagline')}
      label={t('ob.steps')}
      optionalLabel={t('ob.optional')}
      progressLabel={t('ob.progress')}
      onselect={(i) => (step = i)}
    />

    <div class="pane">
      <header class="head">
        <span class="icon" aria-hidden="true"><StepIcon step={current} size={22} /></span>
        <div class="titles">
          <p class="eyebrow">{t('ob.stepOf', { n: step + 1, total: STEPS.length })}</p>
          <h2>{heading}</h2>
          <p class="lead">{lead}</p>
        </div>
      </header>

      <section class="body" aria-live="polite">
        {#if current === 'mic'}
          <MicStep />
        {:else if current === 'voice'}
          <OptionCards
            label={t('ob.voice.title')}
            columns={2}
            bind:value={voice}
            options={VOICES.map((v) => ({
              value: v,
              badge: v,
              title: t(`ob.voice.${v}`),
              description: t(`ob.voice.${v}.desc`),
              tag: profile?.recommendation.voice_profile === v ? t('ob.voice.recommended') : null,
            }))}
          />
        {:else if current === 'hardware'}
          <HardwareStep {profile} {measuring} error={measureError} onretry={() => void measure()} />
        {:else if current === 'keys'}
          {#if wizard}
            <AddProviderWizard
              onfinish={(account) => {
                wizard = false;
                if (account) added++;
              }}
            />
          {:else}
            <div class="panel">
              <h3>{t('ob.keys.cloud.title')}</h3>
              <p class="muted">{t('ob.keys.cloud.desc')}</p>
              {#if added > 0}<p class="ok" role="status">{t('ob.keys.added', { n: added })}</p>{/if}
              <div class="choices">
                <Button variant="primary" onclick={() => (wizard = true)}
                  >{t('ob.keys.addNow')}</Button
                >
                <Button variant="secondary" onclick={next}>{t('ob.keys.skip')}</Button>
              </div>
            </div>
            <div class="panel"><LocalBundleCard /></div>
          {/if}
        {:else if current === 'autonomy'}
          <OptionCards
            label={t('perm.title')}
            bind:value={level}
            options={LEVELS.map((l) => ({
              value: l,
              badge: l,
              title: t(`perm.${l}.name`),
              description: t(`perm.${l}.desc`),
            }))}
          />
          <p class="muted small">{t('perm.kept')}</p>
        {:else if current === 'corpus'}
          <ul class="points">
            {#each ['p1', 'p2', 'p3'] as const as point (point)}
              <li>{t(`ob.corpus.${point}`)}</li>
            {/each}
          </ul>
          <p class="panel muted">{t('ob.corpus.soon')}</p>
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
  </div>
</main>

<style>
  .onboarding {
    display: flex;
    align-items: flex-start;
    justify-content: center;
    height: 100%;
    overflow: auto;
    padding: var(--alfa-space-6) var(--alfa-space-4);
    background: var(--alfa-color-bg);
  }
  .shell {
    display: grid;
    grid-template-columns: 280px minmax(0, 1fr);
    width: min(1040px, 100%);
    /* Stała wysokość (o ile okno pozwala): stopka nie skacze między krokami. `margin: auto`
       w kontenerze flex trzyma kartę na środku, a gdy jest wyższa niż okno — od góry. */
    height: min(680px, 100%);
    min-height: min(520px, 100%);
    margin: auto;
    overflow: hidden;
    border: 1px solid var(--alfa-color-border);
    border-radius: var(--alfa-radius-overlay);
    background: var(--alfa-color-surface);
    box-shadow: var(--alfa-shadow-3);
  }
  .pane {
    display: flex;
    flex-direction: column;
    min-width: 0;
    min-height: 0;
  }
  .head {
    display: flex;
    align-items: flex-start;
    gap: var(--alfa-space-4);
    padding: var(--alfa-space-6) var(--alfa-space-8) var(--alfa-space-4);
  }
  .icon {
    display: inline-flex;
    flex: none;
    align-items: center;
    justify-content: center;
    width: 44px;
    height: 44px;
    border: 1px solid var(--alfa-color-border);
    border-radius: var(--alfa-radius-card);
    background: var(--alfa-color-surface2);
  }
  .titles {
    display: flex;
    flex-direction: column;
    gap: var(--alfa-space-1);
    min-width: 0;
  }
  .eyebrow {
    margin: 0;
    color: var(--alfa-color-text-subtle);
    font-size: var(--alfa-font-size-xs);
    font-weight: var(--alfa-weight-semibold);
    letter-spacing: 0.06em;
    text-transform: uppercase;
  }
  h2 {
    margin: 0;
    font-size: var(--alfa-font-size-xl);
    line-height: 1.25;
  }
  .lead {
    margin: 0;
    color: var(--alfa-color-text-muted);
    line-height: 1.5;
  }
  .body {
    display: flex;
    flex: 1;
    flex-direction: column;
    gap: var(--alfa-space-3);
    min-height: 0;
    overflow: auto;
    padding: var(--alfa-space-2) var(--alfa-space-8) var(--alfa-space-6);
  }
  .panel {
    display: flex;
    flex-direction: column;
    gap: var(--alfa-space-2);
    margin: 0;
    padding: var(--alfa-space-4);
    border: 1px solid var(--alfa-color-border);
    border-radius: var(--alfa-radius-card);
    background: var(--alfa-color-surface2);
  }
  .panel h3 {
    margin: 0;
    font-size: var(--alfa-font-size-md);
  }
  .panel p {
    margin: 0;
  }
  .muted {
    color: var(--alfa-color-text-muted);
  }
  .small {
    margin: 0;
    font-size: var(--alfa-font-size-sm);
  }
  .ok {
    color: var(--alfa-color-success);
    font-weight: var(--alfa-weight-semibold);
  }
  .choices {
    display: flex;
    flex-wrap: wrap;
    gap: var(--alfa-space-2);
    margin-top: var(--alfa-space-1);
  }
  .points {
    display: flex;
    flex-direction: column;
    gap: var(--alfa-space-2);
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .points li {
    display: flex;
    align-items: center;
    gap: var(--alfa-space-3);
  }
  .points li::before {
    content: '';
    flex: none;
    width: 8px;
    height: 8px;
    border-radius: var(--alfa-radius-full);
    background: var(--alfa-color-success);
  }
  .foot {
    display: flex;
    flex: none;
    align-items: center;
    gap: var(--alfa-space-2);
    padding: var(--alfa-space-4) var(--alfa-space-8);
    border-top: 1px solid var(--alfa-color-border);
    background: var(--alfa-color-surface);
  }
  .spacer {
    flex: 1;
  }
  @media (max-width: 760px) {
    .onboarding {
      padding: var(--alfa-space-2);
    }
    .shell {
      grid-template-columns: 1fr;
      grid-template-rows: auto minmax(0, 1fr);
      height: 100%;
    }
    .head {
      padding: var(--alfa-space-4) var(--alfa-space-4) var(--alfa-space-3);
    }
    .body {
      padding: var(--alfa-space-2) var(--alfa-space-4) var(--alfa-space-4);
    }
    .foot {
      padding: var(--alfa-space-3) var(--alfa-space-4);
    }
  }
</style>
