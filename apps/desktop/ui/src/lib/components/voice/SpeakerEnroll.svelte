<!--
  „Rozpoznawanie mojego głosu" (F5): kreator rejestracji — fraza do przeczytania, nagranie
  (mikrofon tylko na czas nagrania), wskaźnik jakości (poziom, długość, spójność) i postęp N/M;
  profil szyfrowany lokalnie, usuwanie z potwierdzeniem. W Ustawieniach — przełącznik
  „wymagaj weryfikacji dla akcji ryzykownych" i wynik ostatniej tury głosowej.
-->
<script lang="ts">
  import { Button, ConfirmDialog, LevelMeter, Switch } from '@alfa/ui-kit';
  import type { VoiceFeatures } from '../../api/types-voice';
  import { levelOf } from '../../logic/voice-features';
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
  const s = $derived(features.speaker);
  const enrolling = $derived(s.state === 'enrolling');
  const prompt = $derived(s.prompts.length ? s.prompts[s.done % s.prompts.length] : null);
  const sample = $derived(s.last_sample);
  let confirmDelete = $state(false);
  let rev = $state(0);

  async function setRequired(required: boolean) {
    await act.speaker({ kind: 'set_required', required });
    rev += 1;
  }
</script>

<section class="vf-card" aria-labelledby="{uid}-title">
  <div class="vf-head">
    <h3 id="{uid}-title">{t('vf.speaker.title')}</h3>
    <span class="vf-badge" class:on={s.state === 'enrolled'}
      >{t(`vf.speaker.state.${s.state}`, { done: s.done, needed: s.needed })}</span
    >
  </div>
  {#if settings}<p class="vf-muted">{t('vf.speaker.desc')}</p>{/if}
  {#if s.reason}<p class="vf-warn" role="status">{app.i18n.text(s.reason)}</p>{/if}

  {#if enrolling}
    <p class="vf-muted">{t('vf.speaker.read')}</p>
    {#if prompt}<p class="vf-prompt" lang={app.i18n.locale}>{app.i18n.text(prompt)}</p>{/if}
    {#if s.recording}<p class="vf-state" role="status">{t('vf.speaker.recording')}</p>{/if}
    <div class="vf-row">
      {#if s.recording}
        <Button
          size="sm"
          variant="primary"
          onclick={() => void act.speaker({ kind: 'record_stop' })}
          >{t('vf.speaker.recordStop')}</Button
        >
      {:else}
        <Button
          size="sm"
          variant="primary"
          onclick={() => void act.speaker({ kind: 'record_start' })}
          >{t('vf.speaker.record')}</Button
        >
        <Button
          size="sm"
          variant="secondary"
          disabled={s.done < s.needed}
          onclick={() => void act.speaker({ kind: 'finish' })}>{t('vf.speaker.finish')}</Button
        >
        <Button size="sm" variant="ghost" onclick={() => void act.speaker({ kind: 'cancel' })}
          >{t('vf.speaker.cancel')}</Button
        >
      {/if}
    </div>
    {#if sample}
      <LevelMeter
        level={levelOf(sample.level_db)}
        label={t('vf.speaker.level')}
        accent={sample.accepted ? 'var(--alfa-color-success)' : 'var(--alfa-color-warning)'}
      />
      <p class:vf-warn={!sample.accepted}>
        {t('vf.speaker.sample', {
          quality: t(`vf.quality.${sample.quality}`),
          secs: (sample.duration_ms / 1000).toFixed(1),
          db: Math.round(sample.level_db),
        })}
      </p>
      {#if sample.message}<p class="vf-muted">{app.i18n.text(sample.message)}</p>{/if}
    {/if}
  {:else if s.state !== 'unavailable'}
    <div class="vf-row">
      <Button
        size="sm"
        variant={s.state === 'enrolled' ? 'secondary' : 'primary'}
        onclick={() => void act.speaker({ kind: 'begin' })}
        >{t(s.state === 'enrolled' ? 'vf.speaker.again' : 'vf.speaker.start')}</Button
      >
      {#if s.state === 'enrolled' && settings}
        <Button size="sm" variant="danger" onclick={() => (confirmDelete = true)}
          >{t('vf.speaker.delete')}</Button
        >
      {/if}
    </div>
  {/if}

  {#if settings}
    {#key rev}
      <div class="vf-toggle">
        <div>
          <span id="{uid}-req">{t('vf.speaker.required')}</span>
          <span class="vf-muted" id="{uid}-req-hint">{t('vf.speaker.requiredHint')}</span>
        </div>
        <Switch
          checked={s.required_for_risky}
          labelledby="{uid}-req"
          describedby="{uid}-req-hint"
          onchange={(on) => void setRequired(on)}
        />
      </div>
    {/key}
  {/if}
  {#if s.last_check}
    <p class="vf-muted">
      {s.last_check.score_permille === null
        ? t('vf.speaker.lastCheck', { decision: t(`vf.decision.${s.last_check.decision}`) })
        : t('vf.speaker.lastCheckScore', {
            decision: t(`vf.decision.${s.last_check.decision}`),
            score: s.last_check.score_permille,
          })}
    </p>
  {/if}
</section>

<ConfirmDialog
  bind:open={confirmDelete}
  title={t('vf.speaker.deleteTitle')}
  description={t('vf.speaker.deleteBody')}
  confirmLabel={t('vf.speaker.delete')}
  cancelLabel={t('common.cancel')}
  danger
  onconfirm={() => {
    confirmDelete = false;
    void act.speaker({ kind: 'delete' });
  }}
/>
