<!--
  Lekkie menu przycisku (wzorzec WAI-ARIA „menu button") bez zależności pozycjonujących:
  strzałki / Home / End, Esc wraca fokusem do przycisku, klik poza menu zamyka.
-->
<script lang="ts" module>
  export interface MenuItem {
    readonly id: string;
    readonly label: string;
    readonly danger?: boolean;
    readonly disabled?: boolean;
    readonly checked?: boolean;
    readonly separatorBefore?: boolean;
    readonly onSelect: () => void;
  }

  export interface MenuTriggerProps {
    'aria-haspopup': 'menu';
    'aria-expanded': boolean;
    'aria-controls': string;
    onclick: (event: MouseEvent) => void;
    onkeydown: (event: KeyboardEvent) => void;
  }
</script>

<script lang="ts">
  import type { Snippet } from 'svelte';
  import { tick } from 'svelte';

  interface Props {
    items: readonly MenuItem[];
    /** Nazwa menu dla czytników ekranu. */
    label: string;
    align?: 'start' | 'end';
    open?: boolean;
    trigger: Snippet<[MenuTriggerProps]>;
  }

  let { items, label, align = 'end', open = $bindable(false), trigger }: Props = $props();

  const menuId = $props.id();
  let anchor = $state<HTMLElement | null>(null);
  let menu = $state<HTMLElement | null>(null);
  let upward = $state(false);

  const enabled = (): HTMLElement[] =>
    Array.from(menu?.querySelectorAll<HTMLElement>('[role^="menuitem"]:not([disabled])') ?? []);

  async function show(focus: 'first' | 'last' = 'first') {
    if (anchor) upward = anchor.getBoundingClientRect().bottom > window.innerHeight * 0.6;
    open = true;
    await tick();
    const list = enabled();
    (focus === 'first' ? list[0] : list[list.length - 1])?.focus();
  }

  function hide(returnFocus = true) {
    open = false;
    if (returnFocus) anchor?.querySelector<HTMLElement>('[aria-haspopup]')?.focus();
  }

  function onTriggerKey(event: KeyboardEvent) {
    if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
      event.preventDefault();
      void show(event.key === 'ArrowDown' ? 'first' : 'last');
    }
  }

  function onMenuKey(event: KeyboardEvent) {
    const list = enabled();
    const at = list.indexOf(document.activeElement as HTMLElement);
    let next = -1;
    if (event.key === 'ArrowDown') next = (at + 1) % list.length;
    else if (event.key === 'ArrowUp') next = (at - 1 + list.length) % list.length;
    else if (event.key === 'Home') next = 0;
    else if (event.key === 'End') next = list.length - 1;
    else if (event.key === 'Escape') {
      event.preventDefault();
      event.stopPropagation();
      hide();
      return;
    } else if (event.key === 'Tab') {
      hide(false);
      return;
    }
    if (next >= 0) {
      event.preventDefault();
      list[next]?.focus();
    }
  }

  function choose(item: MenuItem) {
    hide();
    item.onSelect();
  }

  function onWindowPointer(event: PointerEvent) {
    if (open && anchor && !anchor.contains(event.target as Node)) hide(false);
  }
</script>

<svelte:window onpointerdown={onWindowPointer} />

<span class="anchor" bind:this={anchor}>
  {@render trigger({
    'aria-haspopup': 'menu',
    'aria-expanded': open,
    'aria-controls': menuId,
    onclick: () => (open ? hide() : void show()),
    onkeydown: onTriggerKey,
  })}
  {#if open}
    <div
      class="menu {align}"
      class:upward
      role="menu"
      id={menuId}
      aria-label={label}
      tabindex="-1"
      bind:this={menu}
      onkeydown={onMenuKey}
    >
      {#each items as item (item.id)}
        {#if item.separatorBefore}<div class="sep" role="separator"></div>{/if}
        <button
          type="button"
          role={item.checked === undefined ? 'menuitem' : 'menuitemcheckbox'}
          aria-checked={item.checked}
          class="item"
          class:danger={item.danger}
          tabindex="-1"
          disabled={item.disabled}
          onclick={() => choose(item)}
        >
          {item.label}
        </button>
      {/each}
    </div>
  {/if}
</span>

<style>
  .anchor {
    position: relative;
    display: inline-flex;
  }
  .menu {
    position: absolute;
    top: calc(100% + 4px);
    z-index: 60;
    display: flex;
    flex-direction: column;
    min-width: 200px;
    max-width: 320px;
    padding: var(--alfa-space-1);
    border: 1px solid var(--alfa-color-border);
    border-radius: var(--alfa-radius-card);
    background: var(--alfa-color-surface);
    box-shadow: var(--alfa-shadow-3);
  }
  .menu.upward {
    top: auto;
    bottom: calc(100% + 4px);
  }
  .end {
    right: 0;
  }
  .start {
    left: 0;
  }
  .item {
    display: flex;
    align-items: center;
    min-height: 32px;
    padding: 0 var(--alfa-space-3);
    border: 0;
    border-radius: var(--alfa-radius-control);
    background: transparent;
    color: var(--alfa-color-text);
    font-size: var(--alfa-font-size-sm);
    text-align: left;
    white-space: nowrap;
  }
  .item:hover:not(:disabled),
  .item:focus-visible {
    background: var(--alfa-color-surface3);
  }
  .item[aria-checked='true']::before {
    content: '✓';
    margin-right: var(--alfa-space-2);
  }
  .item:disabled {
    opacity: 0.5;
  }
  .danger {
    color: var(--alfa-color-error);
  }
  .sep {
    height: 1px;
    margin: var(--alfa-space-1) 0;
    background: var(--alfa-color-border);
  }
</style>
