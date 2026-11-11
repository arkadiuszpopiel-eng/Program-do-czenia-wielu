<!--
  Niemodalna nakładka przy przycisku (np. szczegóły kosztów): Esc i klik poza nią zamykają. Przy
  przycisku nisko w oknie (pasek pisania) otwiera się w górę, jak `Menu`.
-->
<script lang="ts" module>
  export interface PopoverTriggerProps {
    'aria-expanded': boolean;
    'aria-controls': string;
    onclick: (event: MouseEvent) => void;
  }
</script>

<script lang="ts">
  import type { Snippet } from 'svelte';
  import { tick } from 'svelte';
  import { uiZoom } from '../zoom';

  interface Props {
    label: string;
    open?: boolean;
    align?: 'start' | 'end';
    width?: number;
    trigger: Snippet<[PopoverTriggerProps]>;
    children: Snippet;
  }

  let {
    label,
    open = $bindable(false),
    align = 'end',
    width = 320,
    trigger,
    children,
  }: Props = $props();

  const panelId = $props.id();
  let anchor = $state<HTMLElement | null>(null);
  let panel = $state<HTMLElement | null>(null);
  let upward = $state(false);
  let maxHeight = $state<number | undefined>();
  let left = $state<number | undefined>();

  // Prostokąt przycisku i okno są w pikselach okna; limit CSS przeliczamy przez zoom.
  $effect(() => {
    if (!open || !anchor || !panel) return;
    const target = anchor;
    const content = panel;
    const update = () => {
      const zoom = uiZoom();
      const rect = target.getBoundingClientRect();
      const inset = 14 * zoom; // 6 px odstępu od przycisku + 8 px od krawędzi okna.
      const above = Math.max(0, rect.top - inset);
      const below = Math.max(0, window.innerHeight - rect.bottom - inset);
      const up = above > below;
      upward = up;
      maxHeight = (up ? above : below) / zoom;
      const panelWidth = content.offsetWidth * zoom;
      const desired = align === 'end' ? rect.right - panelWidth : rect.left;
      const edge = 8 * zoom;
      const bounded = Math.max(edge, Math.min(desired, window.innerWidth - panelWidth - edge));
      left = (bounded - rect.left) / zoom;
    };
    update();
    const resize = new ResizeObserver(update);
    resize.observe(target);
    resize.observe(content); // Także po doładowaniu zawartości przez Lazy.
    resize.observe(document.documentElement);
    const scale = new MutationObserver(update);
    scale.observe(document.documentElement, { attributes: true, attributeFilter: ['style'] });
    window.addEventListener('resize', update);
    return () => {
      resize.disconnect();
      scale.disconnect();
      window.removeEventListener('resize', update);
    };
  });

  async function toggle() {
    open = !open;
    if (open) {
      await tick();
      panel?.focus();
    }
  }

  function close() {
    open = false;
    anchor?.querySelector<HTMLElement>('[aria-controls]')?.focus();
  }

  function onKey(event: KeyboardEvent) {
    if (event.key === 'Escape') {
      event.preventDefault();
      event.stopPropagation();
      close();
    }
  }

  function onWindowPointer(event: PointerEvent) {
    if (open && anchor && !anchor.contains(event.target as Node)) open = false;
  }
</script>

<svelte:window onpointerdown={onWindowPointer} />

<span class="anchor" bind:this={anchor}>
  {@render trigger({ 'aria-expanded': open, 'aria-controls': panelId, onclick: toggle })}
  {#if open}
    <div
      class="popover {align}"
      class:upward
      role="dialog"
      aria-label={label}
      id={panelId}
      tabindex="-1"
      style:width="{width}px"
      style:max-height={maxHeight === undefined ? undefined : `${maxHeight}px`}
      style:left={left === undefined ? undefined : `${left}px`}
      style:right={left === undefined ? undefined : 'auto'}
      bind:this={panel}
      onkeydown={onKey}
    >
      {@render children()}
    </div>
  {/if}
</span>

<style>
  .anchor {
    position: relative;
    display: inline-flex;
  }
  .popover {
    box-sizing: border-box;
    position: absolute;
    top: calc(100% + 6px);
    z-index: 60;
    max-width: calc(var(--alfa-vw, 100vw) - 16px);
    overflow-y: auto;
    scroll-padding-block: var(--alfa-space-2);
    overscroll-behavior: contain;
    padding: var(--alfa-space-3) var(--alfa-space-4);
    border: 1px solid var(--alfa-color-border);
    border-radius: var(--alfa-radius-card);
    background: var(--alfa-color-surface);
    box-shadow: var(--alfa-shadow-3);
    color: var(--alfa-color-text);
    font-size: var(--alfa-font-size-sm);
  }
  .popover {
    transform-origin: top;
    animation: pop var(--alfa-duration-fast) var(--alfa-ease-out);
  }
  .popover.upward {
    top: auto;
    bottom: calc(100% + 6px);
    transform-origin: bottom;
  }
  @keyframes pop {
    from {
      opacity: 0;
      transform: translateY(4px) scale(0.98);
    }
  }
  .popover:focus {
    outline: none;
  }
  .popover:focus-visible {
    outline: var(--alfa-size-focus-ring) solid var(--alfa-color-focus);
  }
  .end {
    right: 0;
  }
  .start {
    left: 0;
  }
</style>
