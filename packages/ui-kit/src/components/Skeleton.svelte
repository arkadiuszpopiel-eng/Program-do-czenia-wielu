<script lang="ts">
  interface Props {
    lines?: number;
    /** Tekst dla czytników ekranu. */
    label?: string;
  }

  let { lines = 3, label = 'Ładowanie' }: Props = $props();
</script>

<div class="skeleton" role="status" aria-label={label}>
  {#each Array.from({ length: lines }, (_, i) => i) as i (i)}
    <span class="line" style:width="{i === lines - 1 ? 60 : 100 - i * 6}%" aria-hidden="true"
    ></span>
  {/each}
</div>

<style>
  .skeleton {
    display: flex;
    flex-direction: column;
    gap: var(--alfa-space-2);
    padding: var(--alfa-space-1) 0;
  }
  .line {
    display: block;
    height: 12px;
    border-radius: var(--alfa-radius-control);
    background: var(--alfa-color-surface3);
    animation: pulse 1.2s var(--alfa-ease-in-out) infinite alternate;
  }
  @keyframes pulse {
    from {
      opacity: 1;
    }
    to {
      opacity: 0.45;
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .line {
      animation: none;
    }
  }
  :global(:root[data-motion='off']) .line {
    animation: none;
  }
</style>
