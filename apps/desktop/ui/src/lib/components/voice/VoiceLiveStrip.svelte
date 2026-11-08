<!--
  Pasek „rozmowa na żywo” nad polem pisania (plan P1 — kula stanu w rozmowie): pojawia się, gdy
  tor głosu coś robi — słyszy użytkownika, przetwarza albo mówi agentka — i pokazuje stan słowem,
  mały pierścień poziomu oraz tekst rozpoznawany na bieżąco. Gdy Alfa tylko czeka, paska nie ma
  (rozmowa nie skacze przy każdym włączeniu mikrofonu).
-->
<script lang="ts">
  import { liveStripVisible, orbRing, orbSpeaker } from '../../logic/voice-orb';
  import { agentName } from '../../logic/work';
  import { useApp } from '../../state/context';

  const app = useApp();
  const { t } = app.i18n;
  const state = $derived(app.micState);
  const pill = $derived(app.voice.pill);
  const visible = $derived(liveStripVisible(state));
  const ring = $derived(orbRing(state, app.micLevel));
  const who = $derived(orbSpeaker(state, pill?.speaker ?? 'nobody'));
  const partial = $derived(state === 'speaking' ? null : (pill?.partial ?? null));
</script>

{#if visible}
  <div class="strip state-{state}" data-testid="voice-live" data-state={state}>
    <span class="orb" aria-hidden="true" style:--ring={ring}></span>
    <span class="state">
      {who === 'agent' && pill
        ? t('live.agent', { name: agentName(pill.agent) })
        : t(`mic.${state}`)}
    </span>
    {#if partial}<span class="partial">„{partial}”</span>{/if}
  </div>
{/if}

<style>
  .strip {
    --orb: var(--alfa-color-success);
    display: flex;
    align-items: center;
    gap: var(--alfa-space-2);
    min-height: 28px;
    padding: 0 var(--alfa-space-3);
    color: var(--alfa-color-text-muted);
    font-size: var(--alfa-font-size-sm);
  }
  .state-processing,
  .state-speaking {
    --orb: var(--alfa-color-focus);
  }
  .orb {
    flex: none;
    width: 12px;
    height: 12px;
    border-radius: 50%;
    background: var(--orb);
    transform: scale(var(--ring, 0.6));
    transition: transform 90ms linear;
  }
  :global([data-motion='off']) .orb {
    transition: none;
  }
  .state {
    flex: none;
    color: var(--alfa-color-text);
    font-weight: var(--alfa-weight-semibold);
  }
  .partial {
    min-width: 0;
    overflow: hidden;
    font-style: italic;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
</style>
