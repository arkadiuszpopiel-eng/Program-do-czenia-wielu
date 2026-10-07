<!-- Niemodalna nakładka przy przycisku (np. szczegóły kosztów): Esc i klik poza nią zamykają. -->
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
      role="dialog"
      aria-label={label}
      id={panelId}
      tabindex="-1"
      style:width="{width}px"
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
    position: absolute;
    top: calc(100% + 6px);
    z-index: 60;
    max-width: calc(100vw - 16px);
    padding: var(--alfa-space-3) var(--alfa-space-4);
    border: 1px solid var(--alfa-color-border);
    border-radius: var(--alfa-radius-card);
    background: var(--alfa-color-surface);
    box-shadow: var(--alfa-shadow-3);
    color: var(--alfa-color-text);
    font-size: var(--alfa-font-size-sm);
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
