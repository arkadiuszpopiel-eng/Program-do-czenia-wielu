<!--
  Stan mikrofonu w pasku tytułu (PLAN §14.9): ikona + tekst (nigdy sam kolor), klik = mikrofon
  wł./wył.; głos niedostępny — przycisk prowadzi do Ustawień → Głos z powodem w podpowiedzi.
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
  const reason = $derived(app.voice.status?.reason ? app.i18n.text(app.voice.status.reason) : '');
  const label = $derived(
    unavailable
      ? t('titlebar.micUnavailable', { reason })
      : t('titlebar.mic', { state: t(`mic.${state}`) }),
  );

  function onclick() {
    if (unavailable) app.openSettings('voice');
    else runCommand(app, 'voice.mic');
  }
</script>

<button
  type="button"
  class="mic state-{state}"
  class:unavailable
  aria-label={label}
  title={label}
  aria-pressed={!unavailable && state !== 'off' && state !== 'muted'}
  aria-keyshortcuts="Control+Shift+M"
  {onclick}
>
  {#if unavailable || state === 'off' || state === 'muted'}
    <MicOff size={14} strokeWidth={1.5} aria-hidden="true" />
  {:else}
    <Mic size={14} strokeWidth={1.5} aria-hidden="true" />
  {/if}
  {#if !unavailable && state !== 'off'}<span class="text">{t(`mic.${state}`)}</span>{/if}
</button>

<style>
  .mic {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    min-height: 28px;
    padding: 0 var(--alfa-space-2);
    border: 0;
    border-radius: var(--alfa-radius-control);
    background: transparent;
    color: var(--alfa-color-text-muted);
    white-space: nowrap;
  }
  .mic:hover {
    background: var(--alfa-color-surface2);
    color: var(--alfa-color-text);
  }
  .mic[aria-pressed='true'] {
    color: var(--alfa-color-text);
  }
  .unavailable {
    color: var(--alfa-color-text-subtle);
  }
  .text {
    font-size: var(--alfa-font-size-xs);
  }
  @media (max-width: 720px) {
    .text {
      display: none;
    }
  }
</style>
