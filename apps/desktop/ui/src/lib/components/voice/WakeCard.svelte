<!--
  Słowa wywoławcze (F5): stan nasłuchu, pomiar FAR/FRR (bez pomiaru — „nieskalibrowane, włączasz
  na własne ryzyko" i dialog potwierdzenia), bramka właściciela, „nie przeszkadzać", test wykrycia
  (bez otwierania rozmowy). W panelu — skrót; w Ustawieniach (`settings`) — pełna konfiguracja.
-->
<script lang="ts">
  import { Button, ConfirmDialog, Switch } from '@alfa/ui-kit';
  import type { VoiceFeatures } from '../../api/types-voice';
  import { calibrationLine, needsRiskConfirmation } from '../../logic/voice-features';
  import { useApp } from '../../state/context';
  import { voiceActions } from './voice-act';

  interface Props {
    features: VoiceFeatures;
    settings?: boolean;
  }

  let { features, settings = false }: Props = $props();
  const app = useApp();
  const { t } = app.i18n;
  const act = voiceActions(app);
  const uid = $props.id();
  const w = $derived(features.wake);
  const cal = $derived(calibrationLine(w.calibration));
  const unavailable = $derived(w.state === 'unavailable');
  let confirmRisk = $state(false);
  /** Po każdej akcji przełączniki odtwarzają stan z rdzenia (także po odmowie). */
  let rev = $state(0);

  async function configure(enabled: boolean, acceptRisk: boolean, ownerGate: boolean) {
    await act.wake({
      kind: 'configure',
      enabled,
      accept_risk: acceptRisk,
      owner_gate: ownerGate,
    });
    rev += 1;
  }

  async function setDnd(on: boolean) {
    await act.wake({ kind: 'set_dnd', on });
    rev += 1;
  }

  function toggleEnabled(on: boolean) {
    if (on && needsRiskConfirmation(w.calibration) && !w.risk_accepted) {
      confirmRisk = true;
      rev += 1;
      return;
    }
    void configure(on, w.risk_accepted, w.owner_gate);
  }
</script>

<section class="vf-card" aria-labelledby="{uid}-title">
  <div class="vf-head">
    <h3 id="{uid}-title">{t('vf.wake.title')}</h3>
    <span class="vf-badge" class:on={w.state === 'armed' || w.state === 'listening'}
      >{t(`vf.wake.state.${w.state}`)}</span
    >
  </div>
  {#if settings}<p class="vf-muted">{t('vf.wake.desc')}</p>{/if}
  {#if w.reason}<p class="vf-warn" role="status">{app.i18n.text(w.reason)}</p>{/if}
  <p class:vf-warn={needsRiskConfirmation(w.calibration)}>{t(cal.key, cal.params)}</p>
  {#if w.phrases.length}
    <p class="vf-muted">{t('vf.wake.phrases', { list: w.phrases.join(' · ') })}</p>
  {/if}
  {#key rev}
    {#if settings}
      <div class="vf-toggle">
        <div>
          <span id="{uid}-enable">{t('vf.wake.enable')}</span>
        </div>
        <Switch
          checked={w.enabled}
          labelledby="{uid}-enable"
          disabled={unavailable}
          onchange={toggleEnabled}
        />
      </div>
      <div class="vf-toggle">
        <div>
          <span id="{uid}-gate">{t('vf.wake.ownerGate')}</span>
          <span class="vf-muted" id="{uid}-gate-hint">{t('vf.wake.ownerGateHint')}</span>
        </div>
        <Switch
          checked={w.owner_gate}
          labelledby="{uid}-gate"
          describedby="{uid}-gate-hint"
          disabled={unavailable}
          onchange={(on) => void configure(w.enabled, w.risk_accepted, on)}
        />
      </div>
    {/if}
    <div class="vf-toggle">
      <div>
        <span id="{uid}-dnd">{t('vf.wake.dnd')}</span>
        <span class="vf-muted" id="{uid}-dnd-hint">{t('vf.wake.dndHint')}</span>
      </div>
      <Switch
        checked={w.dnd}
        labelledby="{uid}-dnd"
        describedby="{uid}-dnd-hint"
        disabled={unavailable}
        onchange={(on) => void setDnd(on)}
      />
    </div>
  {/key}
  <div class="vf-row">
    <Button
      size="sm"
      variant={w.test.active ? 'primary' : 'secondary'}
      disabled={unavailable}
      onclick={() => void act.wake({ kind: 'test', on: !w.test.active })}
      >{t(w.test.active ? 'vf.wake.testStop' : 'vf.wake.test')}</Button
    >
    {#if w.test.active}<span class="vf-muted">{t('vf.wake.testHint')}</span>{/if}
  </div>
  {#if w.test.active || w.test.detections || w.test.owner_rejected}
    <p>
      {w.test.detections
        ? t('vf.wake.testResult', { n: w.test.detections, agent: w.test.last_agent ?? '—' })
        : t('vf.wake.testNone')}
      {#if w.test.owner_rejected}
        · {t('vf.wake.testRejected', { n: w.test.owner_rejected })}{/if}
    </p>
  {/if}
</section>

<ConfirmDialog
  bind:open={confirmRisk}
  title={t('vf.wake.riskTitle')}
  description={t('vf.wake.riskBody')}
  confirmLabel={t('vf.wake.riskConfirm')}
  cancelLabel={t('common.cancel')}
  danger
  onconfirm={() => {
    confirmRisk = false;
    void configure(true, true, w.owner_gate);
  }}
/>
