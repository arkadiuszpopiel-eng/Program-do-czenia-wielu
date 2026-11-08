<script lang="ts">
  import Composer from '../components/Composer.svelte';
  import MicButton from '../components/MicButton.svelte';
  import Chip from '../components/Chip.svelte';
  import type { MicState } from '../types';

  interface Props {
    micState?: MicState;
    sendOnEnter?: boolean;
  }
  let { micState = 'off', sendOnEnter = true }: Props = $props();
  let value = $state('');
  let sent = $state<string[]>([]);
</script>

<div class="demo">
  <Composer bind:value {sendOnEnter} onsubmit={(t) => (sent = [...sent, t])} onattach={() => {}}>
    {#snippet chips()}
      <Chip agent="alfa" size="sm" onclick={() => {}}>Alfa ▾</Chip>
      <Chip size="sm" onclick={() => {}}>Hybryda ▾</Chip>
    {/snippet}
    {#snippet trailing()}
      <MicButton state={micState} showLabel={false} />
    {/snippet}
  </Composer>
  <ul class="sent" aria-label="Wysłane">
    {#each sent as s, i (i)}<li>{s}</li>{/each}
  </ul>
</div>

<style>
  .demo {
    width: min(720px, 90vw);
  }
  .sent {
    margin: var(--alfa-space-3) 0 0;
    padding-left: var(--alfa-space-4);
    color: var(--alfa-color-text-muted);
    font-size: var(--alfa-font-size-sm);
    white-space: pre-wrap;
  }
</style>
