<!-- Przeciągana krawędź panelu (separator ARIA): mysz/pióro/dotyk + klawiatura ←/→, Home/End. -->
<script lang="ts">
  interface Props {
    value: number;
    min: number;
    max: number;
    label: string;
    /** Po której stronie treści jest panel: dla prawego ruch w lewo poszerza. */
    side: 'left' | 'right';
    onchange: (value: number) => void;
  }

  let { value, min, max, label, side, onchange }: Props = $props();
  let start: { x: number; width: number } | null = null;
  let dragging = $state(false);

  const sign = $derived(side === 'left' ? 1 : -1);
  const clamp = (v: number) => Math.min(max, Math.max(min, Math.round(v)));

  function down(event: PointerEvent) {
    if (event.button !== 0) return;
    (event.currentTarget as HTMLElement).setPointerCapture(event.pointerId);
    start = { x: event.clientX, width: value };
    dragging = true;
  }

  function move(event: PointerEvent) {
    if (!start) return;
    onchange(clamp(start.width + sign * (event.clientX - start.x)));
  }

  function up() {
    start = null;
    dragging = false;
  }

  function key(event: KeyboardEvent) {
    const step = event.shiftKey ? 48 : 16;
    let next: number | null = null;
    if (event.key === 'ArrowLeft') next = value - sign * step;
    if (event.key === 'ArrowRight') next = value + sign * step;
    if (event.key === 'Home') next = min;
    if (event.key === 'End') next = max;
    if (next === null) return;
    event.preventDefault();
    onchange(clamp(next));
  }
</script>

<!-- Separator z fokusem to w WAI-ARIA widżet interaktywny („window splitter"). -->
<!-- svelte-ignore a11y_no_noninteractive_tabindex, a11y_no_noninteractive_element_interactions -->
<div
  class="handle {side}"
  class:dragging
  role="separator"
  aria-orientation="vertical"
  aria-label={label}
  aria-valuenow={value}
  aria-valuemin={min}
  aria-valuemax={max}
  tabindex="0"
  onpointerdown={down}
  onpointermove={move}
  onpointerup={up}
  onpointercancel={up}
  onkeydown={key}
></div>

<style>
  .handle {
    position: absolute;
    top: 0;
    bottom: 0;
    z-index: 5;
    width: 8px;
    cursor: col-resize;
    touch-action: none;
  }
  .handle.left {
    right: -4px;
  }
  .handle.right {
    left: -4px;
  }
  .handle::after {
    content: '';
    position: absolute;
    top: 0;
    bottom: 0;
    left: 3px;
    width: 2px;
    background: var(--alfa-color-focus);
    opacity: 0;
    transition: opacity var(--alfa-duration-fast) var(--alfa-ease-out);
  }
  .handle:hover::after,
  .handle.dragging::after,
  .handle:focus-visible::after {
    opacity: 1;
  }
  .handle:focus-visible {
    outline: none;
  }
</style>
