<script lang="ts">
  interface Props {
    /** Skrót w zapisie „Ctrl+Shift+F". */
    keys: string;
  }

  let { keys }: Props = $props();
  const parts = $derived(
    keys.endsWith('++') ? [...keys.slice(0, -2).split('+'), '+'] : keys.split('+'),
  );
</script>

<!-- Czytnik ekranu czyta kolejne klawisze („Ctrl Shift F"); plusy są dekoracją. -->
<span class="kbd">
  {#each parts as part, i (i)}
    {#if i > 0}<span class="plus" aria-hidden="true">+</span>{/if}
    <kbd>{part}</kbd>
  {/each}
</span>

<style>
  .kbd {
    display: inline-flex;
    align-items: center;
    gap: 2px;
    white-space: nowrap;
  }
  kbd {
    min-width: 20px;
    padding: 1px 6px;
    border: 1px solid var(--alfa-color-border);
    border-bottom-width: 2px;
    border-radius: 4px;
    background: var(--alfa-color-surface2);
    color: var(--alfa-color-text);
    font-size: var(--alfa-font-size-xs);
    text-align: center;
  }
  .plus {
    color: var(--alfa-color-text-subtle);
    font-size: var(--alfa-font-size-xs);
  }
</style>
