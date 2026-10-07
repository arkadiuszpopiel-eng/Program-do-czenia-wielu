<!--
  Czytanie na głos (F5): schowek od razu, zaznaczenie z odliczaniem (czas na przejście do okna),
  pasek sterowania (poprzednie, pauza/wznów, następne, stop = Esc, wolniej/szybciej), zdanie n/m,
  kolejka, głos bieżącej agentki. W Ustawieniach — suwak tempa.
-->
<script lang="ts">
  import { Button, Kbd } from '@alfa/ui-kit';
  import type { ReadControlAction, VoiceFeatures } from '../../api/types-voice';
  import { useApp } from '../../state/context';
  import { voiceActions } from './voice-act';

  interface Props {
    features: VoiceFeatures;
    settings?: boolean;
  }

  const COUNTDOWN = 3;

  let { features, settings = false }: Props = $props();
  const app = useApp();
  const { t } = app.i18n;
  const act = voiceActions(app);
  const uid = $props.id();
  const r = $derived(features.read);
  const live = $derived(r.state === 'speaking' || r.state === 'paused');
  let countdown = $state<number | null>(null);
  let rate = $derived(r.rate);

  $effect(() => {
    if (countdown === null) return;
    const handle = setTimeout(() => {
      if (countdown === null) return;
      if (countdown <= 1) {
        countdown = null;
        void act.read({ kind: 'start', source: 'selection' });
      } else countdown -= 1;
    }, 1000);
    return () => clearTimeout(handle);
  });

  const control = (c: ReadControlAction) => void act.read({ kind: 'control', control: c });

  // Po odmowie suwak wraca do tempa z rdzenia (toast pokazuje `voiceActions`).
  async function setRate() {
    if (!(await act.read({ kind: 'set_rate', rate: Number(rate) }))) rate = r.rate;
  }
</script>

<section class="vf-card" aria-labelledby="{uid}-title">
  <div class="vf-head">
    <h3 id="{uid}-title">{t('vf.read.title')}</h3>
    <span class="vf-badge" class:on={r.state === 'speaking'}
      >{t(`vf.read.state.${r.state}`, {
        n: r.index + 1,
        of: r.segments,
        app: r.app ?? '—',
        agent: r.agent,
      })}</span
    >
  </div>
  {#if settings}<p class="vf-muted">{t('vf.read.desc')}</p>{/if}
  {#if r.reason}<p class="vf-warn" role="status">{app.i18n.text(r.reason)}</p>{/if}
  {#if r.queued}<p>{t('vf.read.queued', { n: r.queued })}</p>{/if}
  <div class="vf-row">
    <Button
      size="sm"
      variant="secondary"
      disabled={r.state === 'unavailable'}
      onclick={() => void act.read({ kind: 'start', source: 'clipboard' })}
      >{t('vf.read.clipboard')}</Button
    >
    {#if countdown !== null}
      <span class="vf-state" role="status">{t('vf.dict.countdown', { s: countdown })}</span>
      <Button size="sm" variant="ghost" onclick={() => (countdown = null)}
        >{t('vf.speaker.cancel')}</Button
      >
    {:else}
      <Button
        size="sm"
        variant="secondary"
        disabled={r.state === 'unavailable'}
        onclick={() => (countdown = COUNTDOWN)}>{t('vf.read.selection', { s: COUNTDOWN })}</Button
      >
    {/if}
  </div>
  {#if live}
    <div class="vf-row" role="toolbar" aria-label={t('vf.read.title')}>
      <Button size="sm" variant="ghost" onclick={() => control('previous')}
        >{t('vf.read.previous')}</Button
      >
      {#if r.state === 'paused'}
        <Button size="sm" variant="primary" onclick={() => control('resume')}
          >{t('vf.read.resume')}</Button
        >
      {:else}
        <Button size="sm" variant="secondary" onclick={() => control('pause')}
          >{t('vf.read.pause')}</Button
        >
      {/if}
      <Button size="sm" variant="ghost" onclick={() => control('next')}>{t('vf.read.next')}</Button>
      <Button size="sm" variant="danger" onclick={() => control('stop')}>{t('vf.read.stop')}</Button
      >
      <Button size="sm" variant="ghost" onclick={() => control('slower')}
        >{t('vf.read.slower')}</Button
      >
      <span>{t('vf.read.rate', { rate: r.rate.toFixed(1) })}</span>
      <Button size="sm" variant="ghost" onclick={() => control('faster')}
        >{t('vf.read.faster')}</Button
      >
    </div>
  {/if}
  {#if settings}
    <label class="vf-range">
      <span>{t('vf.read.rateLabel')}</span>
      <input
        type="range"
        min="0.5"
        max="2"
        step="0.1"
        bind:value={rate}
        onchange={() => void setRate()}
      />
      <span>{t('vf.read.rate', { rate: Number(rate).toFixed(1) })}</span>
    </label>
  {/if}
  {#if r.shortcut}
    <p class="vf-muted">{t('vf.read.shortcut', { key: '' })}<Kbd keys={r.shortcut} /></p>
  {/if}
</section>
