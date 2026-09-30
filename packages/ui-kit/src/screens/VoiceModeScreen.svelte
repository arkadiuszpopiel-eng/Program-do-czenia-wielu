<script lang="ts">
  import Keyboard from '@lucide/svelte/icons/keyboard';
  import Users from '@lucide/svelte/icons/users';
  import CircleStop from '@lucide/svelte/icons/circle-stop';
  import VolumeX from '@lucide/svelte/icons/volume-x';
  import VoiceOrb from './VoiceOrb.svelte';
  import LiveCaptions from './LiveCaptions.svelte';
  import MicButton from '../components/MicButton.svelte';
  import Button from '../components/Button.svelte';
  import Avatar from '../components/Avatar.svelte';
  import { agents, type AgentId } from '../tokens';
  import type { CaptionLine, MicState } from '../types';

  interface Props {
    agent: AgentId;
    micState: MicState;
    lines: readonly CaptionLine[];
    /** Fikcyjny poziom głośności 0–1; gdy `simulate`, generowany lokalnie. */
    level?: number;
    simulate?: boolean;
    ontext?: () => void;
    onstop?: () => void;
  }

  let { agent, micState, lines, level = 0, simulate = false, ontext, onstop }: Props = $props();

  let simLevel = $state(0);
  const shownLevel = $derived(simulate ? simLevel : level);

  $effect(() => {
    if (!simulate) return;
    const speaking = micState === 'speaking' || micState === 'hearing';
    let t = 0;
    const id = setInterval(() => {
      t += 1;
      // Pseudo-głośność: obwiednia „mowy" (sylaby ~4 Hz) z pauzami.
      const env = speaking
        ? Math.max(0, Math.sin(t * 0.8) * 0.5 + 0.5) * (0.6 + 0.4 * Math.abs(Math.sin(t * 0.13)))
        : 0.05;
      simLevel = Math.min(1, env * (0.7 + Math.random() * 0.3));
    }, 1000 / 30);
    return () => clearInterval(id);
  });
</script>

<main class="voice" style:--accent="var(--alfa-agent-{agent})">
  <header class="top">
    <span class="who"
      ><Avatar {agent} size={24} speaking={micState === 'speaking'} />
      {agents[agent].name} · Mówczyni</span
    >
    <MicButton state={micState} {agent} level={shownLevel} />
  </header>

  <div class="stage">
    <VoiceOrb {agent} level={shownLevel} size={240} />
  </div>

  <div class="captions">
    <LiveCaptions {lines} />
  </div>

  <footer class="controls" aria-label="Sterowanie trybem głosowym">
    <Button variant="secondary" onclick={onstop} aria-keyshortcuts="Escape">
      {#snippet icon()}<CircleStop size={16} strokeWidth={1.5} />{/snippet}
      Stop mowy
    </Button>
    <Button variant="secondary" aria-keyshortcuts="Control+Shift+M">
      {#snippet icon()}<VolumeX size={16} strokeWidth={1.5} />{/snippet}
      Wycisz
    </Button>
    <Button variant="secondary">
      {#snippet icon()}<Users size={16} strokeWidth={1.5} />{/snippet}
      Przełącz agentkę
    </Button>
    <Button variant="ghost" onclick={ontext}>
      {#snippet icon()}<Keyboard size={16} strokeWidth={1.5} />{/snippet}
      Przejdź do tekstu
    </Button>
  </footer>
</main>

<style>
  .voice {
    display: grid;
    grid-template-rows: auto 1fr auto auto;
    justify-items: center;
    gap: var(--alfa-space-6);
    height: 100%;
    min-height: 560px;
    padding: var(--alfa-space-4) var(--alfa-space-6) var(--alfa-space-6);
    background: var(--alfa-color-bg);
    color: var(--alfa-color-text);
  }
  .top {
    display: flex;
    align-items: center;
    justify-content: space-between;
    width: 100%;
    max-width: 760px;
  }
  .who {
    display: inline-flex;
    align-items: center;
    gap: var(--alfa-space-2);
    color: var(--accent);
    font-weight: var(--alfa-weight-semibold);
    font-size: var(--alfa-font-size-sm);
  }
  .stage {
    display: flex;
    align-items: center;
    justify-content: center;
  }
  .captions {
    min-height: 120px;
  }
  .controls {
    display: flex;
    flex-wrap: wrap;
    justify-content: center;
    gap: var(--alfa-space-2);
  }
</style>
