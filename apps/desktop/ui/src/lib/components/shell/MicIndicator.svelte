<!--
  Stan mikrofonu w pasku tytułu (PLAN §14.9): ikona + tekst (nigdy sam kolor), klik = mikrofon
  wł./wył.; głos niedostępny — przycisk prowadzi do Ustawień → Głos z powodem w podpowiedzi.
  Włączony mikrofon ma wypełnioną kapsułę i słupki poziomu, które ruszają się, gdy Alfa Cię słyszy.
-->
<script lang="ts">
  import Mic from '@lucide/svelte/icons/mic';
  import MicOff from '@lucide/svelte/icons/mic-off';
  import { useApp } from '../../state/context';
  import { runCommand } from '../../state/commands';

  const app = useApp();
  const { t } = app.i18n;
  const state = $derived(app.micState);
  const unavailable = $derived(!app.voice.available);
  const live = $derived(!unavailable && state !== 'off' && state !== 'muted');
  const reason = $derived(app.voice.status?.reason ? app.i18n.text(app.voice.status.reason) : '');
  const label = $derived(
    unavailable
      ? t('titlebar.micUnavailable', { reason })
      : t('titlebar.mic', { state: t(`mic.${state}`) }),
  );
  // Trzy słupki: środkowy najwyższy; wysokość rośnie z poziomem sygnału.
  const level = $derived(Math.max(0, Math.min(1, app.micLevel)));
  const bars = $derived([0.55, 1, 0.7].map((weight) => 0.25 + 0.75 * level * weight));

  function onclick() {
    if (unavailable) app.openSettings('voice');
    else runCommand(app, 'voice.mic');
  }
</script>

<button
  type="button"
  class="mic state-{state}"
  class:unavailable
  class:live
  aria-label={label}
  title={label}
  aria-pressed={live}
  aria-keyshortcuts="Control+Shift+M"
  {onclick}
>
  {#if live}
    <Mic size={15} strokeWidth={1.75} aria-hidden="true" />
    <span class="bars" aria-hidden="true">
      {#each bars as height, i (i)}<span style:transform="scaleY({height})"></span>{/each}
    </span>
  {:else}
    <MicOff size={15} strokeWidth={1.75} aria-hidden="true" />
  {/if}
  {#if !unavailable && state !== 'off'}<span class="text">{t(`mic.${state}`)}</span>{/if}
  {#if unavailable}<span class="mark" aria-hidden="true">!</span>{/if}
</button>

<style>
  .mic {
    position: relative;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 5px;
    min-width: 30px;
    height: 30px;
    padding: 0 var(--alfa-space-2);
    border: 1px solid var(--alfa-color-border);
    border-radius: var(--alfa-radius-full);
    background: var(--alfa-color-bg);
    color: var(--alfa-color-text-muted);
    white-space: nowrap;
  }
  .mic:hover {
    border-color: var(--alfa-color-border-strong);
    background: var(--alfa-color-surface2);
    color: var(--alfa-color-text);
  }
  .mic.live {
    border-color: var(--alfa-color-success);
    color: var(--alfa-color-text);
  }
  .mic.live :global(svg) {
    color: var(--alfa-color-success);
  }
  .mic.state-muted,
  .mic.state-dnd {
    border-color: var(--alfa-color-warning);
    color: var(--alfa-color-text);
  }
  .unavailable {
    border-style: dashed;
  }
  .bars {
    display: inline-flex;
    align-items: center;
    gap: 2px;
    height: 14px;
  }
  .bars span {
    width: 2px;
    height: 100%;
    border-radius: 1px;
    background: var(--alfa-color-success);
    transition: transform 90ms linear;
  }
  .text {
    font-size: var(--alfa-font-size-xs);
  }
  .mark {
    position: absolute;
    top: -3px;
    right: -3px;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 13px;
    height: 13px;
    border-radius: var(--alfa-radius-full);
    background: var(--alfa-color-warning);
    color: var(--alfa-color-bg);
    font-size: 10px;
    font-weight: var(--alfa-weight-semibold);
    line-height: 1;
  }
  @media (max-width: 720px) {
    .text,
    .bars {
      display: none;
    }
  }
</style>
