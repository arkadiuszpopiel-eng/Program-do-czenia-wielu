<script lang="ts">
  import Send from '@lucide/svelte/icons/send';
  import Button from '../components/Button.svelte';
  import IconButton from '../components/IconButton.svelte';
  import Settings from '@lucide/svelte/icons/settings';
  import Mic from '@lucide/svelte/icons/mic';
  import { agentIds } from '../tokens';
</script>

<div class="rows">
  <div class="row">
    <Button variant="primary">Wyślij</Button>
    <Button variant="secondary">Anuluj</Button>
    <Button variant="ghost">Więcej</Button>
    <Button variant="danger">Usuń</Button>
    <Button variant="primary" loading>Pracuję</Button>
    <Button variant="secondary" disabled>Wyłączony</Button>
  </div>
  <div class="row">
    {#each agentIds as id (id)}
      <Button variant="primary" agent={id}>
        {#snippet icon()}<Send size={14} strokeWidth={1.5} />{/snippet}
        {id}
      </Button>
    {/each}
  </div>
  <div class="row">
    <Button size="sm" variant="secondary">Mały</Button>
    <Button size="sm" variant="ghost" agent="gama">Ghost Gama</Button>
    <IconButton label="Ustawienia"><Settings size={16} strokeWidth={1.5} /></IconButton>
    <IconButton label="Mikrofon" pressed><Mic size={16} strokeWidth={1.5} /></IconButton>
    <IconButton label="Mały" size="sm"><Settings size={14} strokeWidth={1.5} /></IconButton>
    <IconButton label="Duży" size="lg"><Settings size={20} strokeWidth={1.5} /></IconButton>
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
