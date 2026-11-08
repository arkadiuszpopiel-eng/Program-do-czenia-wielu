<!--
  Kula stanu rozmowy (plan P1): jedno miejsce, w którym widać, co robi tor głosu — słucha, słyszy
  Cię, przetwarza, mówi agentka, wyciszony. Stan jest podany słowem (kula i kolor to dodatek),
  pod nim kto mówi i to, co Alfa właśnie rozpoznaje. Pierścień rośnie z poziomem sygnału;
  przy wyłączonych animacjach kula stoi, a poziom pokazuje sam pierścień.
-->
<script lang="ts">
  import { orbRing, orbSpeaker } from '../../logic/voice-orb';
  import { agentName } from '../../logic/work';
  import { useApp } from '../../state/context';

  const app = useApp();
  const { t } = app.i18n;
  const state = $derived(app.micState);
  const pill = $derived(app.voice.pill);
  const ring = $derived(orbRing(state, app.micLevel));
  const who = $derived(orbSpeaker(state, pill?.speaker ?? 'nobody'));
  const partial = $derived(state === 'hearing' || state === 'processing' ? pill?.partial : null);
</script>

<div class="orb-row state-{state}" data-testid="voice-orb" data-state={state}>
  <span class="orb" aria-hidden="true" style:--ring={ring}>
    <span class="core"></span>
  </span>
  <div class="text">
    <p class="state">{t(`mic.${state}`)}</p>
    {#if who === 'agent' && pill}
      <p class="who">{t('orb.agent', { name: agentName(pill.agent) })}</p>
    {:else if who === 'user'}
      <p class="who">{t('orb.user')}</p>
    {:else}
      <p class="who">{t(`orb.hint.${state}`)}</p>
    {/if}
    {#if partial}<p class="partial">„{partial}”</p>{/if}
  </div>
</div>

<style>
  .orb-row {
    --orb: var(--alfa-color-text-muted);
    display: flex;
    align-items: center;
    gap: var(--alfa-space-3);
    min-height: 56px;
    margin-bottom: var(--alfa-space-2);
  }
  .state-listening,
  .state-hearing {
    --orb: var(--alfa-color-success);
  }
  .state-processing,
  .state-speaking {
    --orb: var(--alfa-color-focus);
  }
  .state-muted,
  .state-dnd {
    --orb: var(--alfa-color-warning);
  }
  .orb {
    position: relative;
    flex: none;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 48px;
    height: 48px;
  }
  /* Pierścień: skala z poziomu sygnału (0,6–1). */
  .orb::before {
    content: '';
    position: absolute;
    inset: 0;
    border: 2px solid var(--orb);
    border-radius: 50%;
    opacity: 0.55;
    transform: scale(var(--ring, 0.6));
    transition: transform 90ms linear;
  }
  .core {
    width: 22px;
    height: 22px;
    border-radius: 50%;
    background: var(--orb);
  }
  .state-off .core {
    background: transparent;
    border: 2px solid var(--orb);
  }
  .state-processing .core {
    animation: orb-think 1.1s ease-in-out infinite;
  }
  .state-speaking .core {
    animation: orb-speak 0.7s ease-in-out infinite;
  }
  @keyframes orb-think {
    50% {
      transform: scale(0.72);
      opacity: 0.6;
    }
  }
  @keyframes orb-speak {
    50% {
      transform: scale(1.18);
    }
  }
  :global([data-motion='off']) .core {
    animation: none;
  }
  :global([data-motion='off']) .orb::before {
    transition: none;
  }
  @media (prefers-reduced-motion: reduce) {
    .core {
      animation: none;
    }
  }
  .text {
    min-width: 0;
  }
  .state {
    margin: 0;
    font-weight: var(--alfa-weight-semibold);
  }
  .who,
  .partial {
    margin: 2px 0 0;
    color: var(--alfa-color-text-muted);
    font-size: var(--alfa-font-size-xs);
  }
  .partial {
    overflow: hidden;
    display: -webkit-box;
    -webkit-box-orient: vertical;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    overflow-wrap: anywhere;
    font-style: italic;
  }
</style>
