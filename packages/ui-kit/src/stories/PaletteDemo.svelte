<script lang="ts">
  import CommandPalette from '../components/CommandPalette.svelte';
  import Button from '../components/Button.svelte';
  import { mockCommands } from '../mock';
  let open = $state(false);
  let last = $state('');
  const items = mockCommands.map((c) => ({ ...c, onSelect: () => (last = c.label) }));
</script>

<div class="demo">
  <Button variant="primary" onclick={() => (open = true)}>Otwórz paletę (Ctrl+K)</Button>
  <p>Ostatnio wybrane: <strong>{last || '—'}</strong></p>
  <CommandPalette bind:open {items} />
</div>

<style>
  .demo {
    display: flex;
    flex-direction: column;
    gap: var(--alfa-space-3);
    align-items: center;
  }
  p {
    color: var(--alfa-color-text-muted);
    font-size: var(--alfa-font-size-sm);
  }
</style>
