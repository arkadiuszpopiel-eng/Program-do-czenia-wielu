<!-- Przełącznik wariantów / gałęzi odpowiedzi: ‹ 1/3 › (PLAN §14.8, ADR 0006). -->
<script lang="ts">
  import ChevronLeft from '@lucide/svelte/icons/chevron-left';
  import ChevronRight from '@lucide/svelte/icons/chevron-right';

  interface Props {
    index: number;
    total: number;
    /** Pełny opis dla czytników, np. „Wariant 1 z 3". */
    label: string;
    prevLabel?: string;
    nextLabel?: string;
    onprev: () => void;
    onnext: () => void;
  }

  let {
    index,
    total,
    label,
    prevLabel = 'Poprzedni',
    nextLabel = 'Następny',
    onprev,
    onnext,
  }: Props = $props();
</script>

<span class="variants" role="group" aria-label={label}>
  <button type="button" class="nav" aria-label={prevLabel} title={prevLabel} onclick={onprev}>
    <ChevronLeft size={14} strokeWidth={1.5} aria-hidden="true" />
  </button>
  <span class="count" aria-hidden="true">{index}/{total}</span>
  <button type="button" class="nav" aria-label={nextLabel} title={nextLabel} onclick={onnext}>
    <ChevronRight size={14} strokeWidth={1.5} aria-hidden="true" />
  </button>
</span>

<style>
  .variants {
    display: inline-flex;
    align-items: center;
    gap: 2px;
    color: var(--alfa-color-text-muted);
    font-size: var(--alfa-font-size-xs);
    font-variant-numeric: tabular-nums;
  }
  .nav {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 24px;
    height: 24px;
    padding: 0;
    border: 0;
    border-radius: var(--alfa-radius-control);
    background: transparent;
    color: inherit;
  }
  .nav:hover {
    background: var(--alfa-color-surface2);
    color: var(--alfa-color-text);
  }
</style>
