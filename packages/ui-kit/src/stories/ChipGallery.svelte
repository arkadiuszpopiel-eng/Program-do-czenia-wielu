<script lang="ts">
  import Chip from '../components/Chip.svelte';
  import Avatar from '../components/Avatar.svelte';
  import { agentIds } from '../tokens';
  let selected = $state('alfa');
</script>

<div class="rows">
  <div class="row">
    <Chip>Neutralny</Chip>
    <Chip tone="success">Sukces</Chip>
    <Chip tone="warning">Ostrzeżenie</Chip>
    <Chip tone="error">Błąd</Chip>
    <Chip tone="info">Info</Chip>
    <Chip size="sm" agent="delta">Delta · Wykonawczyni</Chip>
  </div>
  <div class="row" role="group" aria-label="Wybór agentki">
    {#each agentIds as id (id)}
      <Chip agent={id} selected={selected === id} onclick={() => (selected = id)}>
        {#snippet icon()}<Avatar agent={id} size={20} />{/snippet}
        {id}
      </Chip>
    {/each}
  </div>
  <div class="row">
    {#each agentIds as id (id)}<Avatar agent={id} size={40} />{/each}
    <Avatar agent="beta" size={40} speaking />
    <Avatar agent="gama" size={40} working />
    <Avatar agent="alfa" size={64} speaking />
    <Avatar agent="delta" size={20} />
  </div>
</div>

<style>
  .rows {
    display: flex;
    flex-direction: column;
    gap: var(--alfa-space-4);
  }
  .row {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--alfa-space-2);
  }
</style>
