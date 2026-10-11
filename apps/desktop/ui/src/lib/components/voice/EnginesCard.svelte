<!--
  Silniki rozmowy (Ustawienia → Głos): który silnik rozpoznaje mowę i czy pracuje na karcie,
  strażnik z bezpiecznikiem i skrótem telemetrii, co ocenia koniec wypowiedzi, które głosy są
  gotowe. Brakujące pozycje prowadzą do „Modele lokalne i silniki".
-->
<script lang="ts">
  import { Button } from '@alfa/ui-kit';
  import type { VoiceFeatures } from '../../api/types-voice';
  import { useApp } from '../../state/context';
  import { voiceActions } from './voice-act';

  interface Props {
    features: VoiceFeatures;
  }

  let { features }: Props = $props();
  const app = useApp();
  const { t } = app.i18n;
  const act = voiceActions(app);
  const uid = $props.id();
  const e = $derived(features.engines);
  const guard = $derived(e.stt.guard);
  const activeTts = $derived(e.tts.find((x) => x.ready)?.engine ?? null);
  let busy = $state(false);

  async function run(kind: 'refresh' | 'reset_breaker') {
    busy = true;
    await act.engines({ kind });
    busy = false;
  }

  // Nazwa silnika mowy; nieznany silnik (nowszy rdzeń) pokazujemy jego nazwą techniczną.
  function ttsName(engine: string): string {
    if (engine === 'supertonic') return t('vf.eng.tts.supertonic');
    if (engine === 'pocket') return t('vf.eng.tts.pocket');
    if (engine === 'piper') return t('vf.eng.tts.piper');
    return engine;
  }

  // Nazwy pozycji katalogu zamiast identyfikatorów; nieznaną pozycję pokazujemy identyfikatorem.
  function itemName(id: string): string {
    switch (id) {
      case 'qwen3-asr-1.7b-q8':
        return t('vf.eng.item.qwen');
      case 'smart-turn-v3.2':
        return t('vf.eng.item.smartTurn');
      case 'onnxruntime-cpu':
        return t('vf.eng.item.onnxruntime');
      case 'sidecar-llama-cpu':
        return t('vf.eng.item.llama');
      case 'sidecar-whisper-cpu':
        return t('vf.eng.item.whisperServer');
      case 'whisper-small-q5_1':
        return t('vf.eng.item.whisperSmall');
      case 'alfa-smart-turn':
        return t('vf.eng.item.smartTurnProgram');
      default:
        return id;
    }
  }
  const names = (ids: readonly string[]) => ids.map(itemName).join(', ');

  const ms = (value: number | null) => (value === null ? '—' : t('vf.eng.ms', { n: value }));
</script>

<section class="vf-card" aria-labelledby="{uid}-title" data-testid="voice-engines">
  <div class="vf-head">
    <h3 id="{uid}-title">{t('vf.eng.title')}</h3>
    <Button size="sm" variant="ghost" disabled={busy} onclick={() => void run('refresh')}
      >{t('vf.eng.refresh')}</Button
    >
  </div>
  <p class="vf-muted">{t('vf.eng.desc')}</p>

  <dl class="vf-kv">
    <dt>{t('vf.eng.stt')}</dt>
    <dd>
      <span class="vf-state">{t(`vf.eng.stt.${e.stt.engine}`)}</span>
      {#if e.stt.engine !== 'none'}
        <span class="vf-badge">{t(e.stt.gpu ? 'vf.eng.gpu' : 'vf.eng.cpu')}</span>
      {/if}
      {#if e.stt.fallback}
        <span class="vf-muted"
          >{t('vf.eng.fallback', { engine: t(`vf.eng.stt.${e.stt.fallback}`) })}</span
        >
      {/if}
    </dd>

    <dt>{t('vf.eng.turn')}</dt>
    <dd>
      <span class="vf-state">{t(`vf.eng.turn.${e.turn.engine}`)}</span>
      <span class="vf-muted">{t(`vf.eng.turn.${e.turn.engine}.hint`)}</span>
      {#if e.turn.stats}
        <span class="vf-muted" data-testid="voice-turn-stats"
          >{t('vf.eng.turn.stats', {
            n: e.turn.stats.scored,
            pct: e.turn.stats.last_pct ?? '—',
            ms: e.turn.stats.last_ms ?? '—',
          })}{e.turn.stats.failed > 0
            ? ` ${t('vf.eng.turn.failed', { n: e.turn.stats.failed })}`
            : ''}</span
        >
      {/if}
    </dd>

    <dt>{t('vf.eng.tts')}</dt>
    <dd>
      {#each e.tts as voice (voice.engine)}
        <span class="vf-badge" class:on={voice.engine === activeTts} class:off={!voice.ready}
          >{ttsName(voice.engine)} · {t(
            voice.engine === activeTts
              ? 'vf.eng.tts.active'
              : voice.ready
                ? 'vf.eng.tts.ready'
                : 'vf.eng.tts.missing',
          )}</span
        >
      {/each}
    </dd>
  </dl>

  {#if e.stt.missing.length > 0 || e.turn.missing.length > 0}
    <div class="vf-warn" role="status">
      {#if e.stt.missing.length > 0}
        <p>{t('vf.eng.missing.stt', { items: names(e.stt.missing) })}</p>
      {/if}
      {#if e.turn.missing.length > 0}
        <p>{t('vf.eng.missing.turn', { items: names(e.turn.missing) })}</p>
      {/if}
      <Button size="sm" variant="secondary" onclick={() => app.openSettings('models')}
        >{t('vf.eng.openModels')}</Button
      >
    </div>
  {/if}

  {#if guard}
    <div class="vf-guard" data-testid="voice-guard">
      <div class="vf-head">
        <h4>{t('vf.eng.guard')}</h4>
        <span
          class="vf-badge"
          class:on={guard.breaker === 'closed'}
          class:alert={guard.breaker !== 'closed'}
          data-testid="voice-breaker">{t(`vf.eng.breaker.${guard.breaker}`)}</span
        >
      </div>
      {#if guard.breaker !== 'closed'}
        <p class="vf-warn" role="status">
          {t('vf.eng.breaker.note', {
            engine: t(`vf.eng.stt.${e.stt.fallback ?? 'whisper'}`),
            s: guard.retry_in_s ?? 0,
          })}
        </p>
        <div class="vf-row">
          <Button
            size="sm"
            variant="primary"
            disabled={busy}
            onclick={() => void run('reset_breaker')}>{t('vf.eng.breaker.reset')}</Button
          >
        </div>
      {/if}
      <dl class="vf-stats">
        <div>
          <dt>{t('vf.eng.stat.samples')}</dt>
          <dd>{guard.samples}</dd>
        </div>
        <div>
          <dt>{t('vf.eng.stat.median')}</dt>
          <dd>{ms(guard.median_ms)}</dd>
        </div>
        <div>
          <dt>{t('vf.eng.stat.p95')}</dt>
          <dd>{ms(guard.p95_ms)}</dd>
        </div>
        <div>
          <dt>{t('vf.eng.stat.fallbacks')}</dt>
          <dd>{guard.fallbacks}</dd>
        </div>
        <div>
          <dt>{t('vf.eng.stat.flagged')}</dt>
          <dd>{guard.flagged}</dd>
        </div>
        <div>
          <dt>{t('vf.eng.stat.failures')}</dt>
          <dd>{guard.failures} / {guard.failures_to_open}</dd>
        </div>
        <div>
          <dt>{t('vf.eng.stat.opened')}</dt>
          <dd>{guard.opened_count}</dd>
        </div>
      </dl>
    </div>
  {:else if e.stt.engine === 'qwen_asr'}
    <p class="vf-muted">{t('vf.eng.guard.idle')}</p>
  {/if}
</section>
