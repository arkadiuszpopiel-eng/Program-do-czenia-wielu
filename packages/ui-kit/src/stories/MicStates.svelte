<script lang="ts">
  import MicButton from '../components/MicButton.svelte';
  import type { MicState } from '../types';
  const states: MicState[] = [
    'off',
    'listening',
    'hearing',
    'processing',
    'speaking',
    'muted',
    'dnd',
  ];
</script>

<div class="grid">
  {#each states as s (s)}
    <div class="cell">
      <MicButton state={s} agent="beta" level={0.6} />
      <MicButton state={s} agent="beta" level={0.6} showLabel={false} />
      <code>{s}</code>
    </div>
  {/each}
</div>

<style>
  .grid {
    display: grid;
    grid-template-columns: repeat(2, auto);
    gap: var(--alfa-space-3) var(--alfa-space-6);
  }
  .cell {
    display: flex;
    align-items: center;
    gap: var(--alfa-space-3);
  }
  code {
    color: var(--alfa-color-text-subtle);
  }
</style>
