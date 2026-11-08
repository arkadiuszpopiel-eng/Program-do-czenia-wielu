<!--
  Pasek LED wzdłuż dolnej krawędzi belki tytułu: listwa w barwach agentek i światło jadące od lewej
  do prawej — szybciej, gdy któraś agentka pracuje albo mówi; bez połączenia — barwa ostrzeżenia.
-->
<script lang="ts">
  import { useApp } from '../../state/context';

  const app = useApp();
  const agentStates = $derived(app.activeId ? (app.agents[app.activeId] ?? []) : []);
  const busy = $derived(
    agentStates.some((a) => a.status === 'working' || a.status === 'speaking') ||
      app.conversation?.streaming !== undefined,
  );
</script>

<div class="led" class:busy class:offline={app.system?.online === false} aria-hidden="true"></div>

<style>
  /* Pasek LED ma własny tor pod elementami belki (`--led-lane`): listwa w barwach agentek i jasne
     światło jadące od lewej do prawej. Nic z paska nie wchodzi na przyciski i kapsuły. */
  .led {
    position: absolute;
    left: 0;
    right: 0;
    bottom: 0;
    height: 3px;
    overflow: hidden;
    pointer-events: none;
    /* Barwy agentek rozjaśnione do świecących — czytelne na jasnej i ciemnej belce. */
    --led-a: oklch(66% 0.23 27);
    --led-b: oklch(66% 0.22 292);
    --led-c: oklch(72% 0.15 215);
    --led-d: oklch(72% 0.17 162);
  }
  .led::before {
    content: '';
    position: absolute;
    inset: 0;
    background: linear-gradient(90deg, var(--led-a), var(--led-b), var(--led-c), var(--led-d));
    opacity: 0.38;
  }
  .led::after {
    content: '';
    position: absolute;
    top: 0;
    bottom: 0;
    left: 0;
    width: 28%;
    background:
      linear-gradient(90deg, transparent 55%, #ffffff 82%, transparent 97%) center / 100% 1px
        no-repeat,
      linear-gradient(
        90deg,
        transparent,
        var(--led-a) 22%,
        var(--led-b) 50%,
        var(--led-c) 76%,
        var(--led-d) 92%,
        transparent
      );
    transform: translateX(-100%);
    animation: led-run 3.2s linear infinite;
  }
  .led.busy::after {
    animation-duration: 1.4s;
  }
  .led.offline {
    --led-a: var(--alfa-color-warning);
    --led-b: var(--alfa-color-warning);
    --led-c: var(--alfa-color-warning);
    --led-d: var(--alfa-color-warning);
  }
  @keyframes led-run {
    to {
      transform: translateX(358%);
    }
  }
  /* Animacje wyłączone: światło stoi — zostaje równa, jaśniejsza listwa. */
  :global(:root[data-motion='off']) .led::after {
    display: none;
  }
  :global(:root[data-motion='off']) .led::before {
    opacity: 0.85;
  }
  @media (prefers-reduced-motion: reduce) {
    .led::after {
      display: none;
    }
    .led::before {
      opacity: 0.85;
    }
  }
  @media (forced-colors: active) {
    .led {
      display: none;
    }
  }
</style>
