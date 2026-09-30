<!--
  Paleta poleceń (Ctrl+K). UWAGA: to JEDYNY plik w repo, w którym dozwolony jest
  `backdrop-filter` (PLAN.md §14.3 / §14.7). Dostępność i pułapka fokusu: bits-ui Dialog + Command.
-->
<script lang="ts">
  import { Command, Dialog } from 'bits-ui';
  import Search from '@lucide/svelte/icons/search';
  import type { CommandItem } from '../types';

  interface Props {
    /** Czy paleta jest otwarta (bindable). */
    open?: boolean;
    items: readonly CommandItem[];
    placeholder?: string;
    /** Rejestruj globalny skrót Ctrl+K (domyślnie tak). */
    hotkey?: boolean;
  }

  let {
    open = $bindable(false),
    items,
    placeholder = 'Wpisz polecenie…',
    hotkey = true,
  }: Props = $props();

  /** Grupy w kolejności pierwszego wystąpienia (lokalna, niereaktywna struktura). */
  const groups = $derived.by(() => {
    const order: string[] = [];
    const byGroup: Record<string, CommandItem[]> = {};
    for (const item of items) {
      if (!byGroup[item.group]) {
        byGroup[item.group] = [];
        order.push(item.group);
      }
      byGroup[item.group]?.push(item);
    }
    return order.map((g): [string, CommandItem[]] => [g, byGroup[g] ?? []]);
  });

  function onWindowKeydown(event: KeyboardEvent) {
    if (!hotkey) return;
    if ((event.ctrlKey || event.metaKey) && !event.altKey && event.key.toLowerCase() === 'k') {
      event.preventDefault();
      open = !open;
    }
  }

  function select(item: CommandItem) {
    open = false;
    item.onSelect?.();
  }
</script>

<svelte:window onkeydown={onWindowKeydown} />

<Dialog.Root bind:open>
  <Dialog.Portal>
    <Dialog.Overlay class="alfa-palette-overlay" />
    <Dialog.Content class="alfa-palette" aria-describedby={undefined}>
      <Dialog.Title class="alfa-visually-hidden">Paleta poleceń</Dialog.Title>
      <Command.Root class="alfa-palette-cmd" loop>
        <div class="input-row">
          <Search size={16} strokeWidth={1.5} aria-hidden="true" />
          <Command.Input class="alfa-palette-input" {placeholder} aria-label="Szukaj polecenia" />
          <kbd class="esc">Esc</kbd>
        </div>
        <Command.List class="alfa-palette-list">
          <Command.Viewport>
            <Command.Empty class="alfa-palette-empty"
              >Brak poleceń pasujących do zapytania.</Command.Empty
            >
            {#each groups as [group, list] (group)}
              <Command.Group class="alfa-palette-group">
                <Command.GroupHeading class="alfa-palette-heading">{group}</Command.GroupHeading>
                <Command.GroupItems>
                  {#each list as item (item.id)}
                    <Command.Item
                      class="alfa-palette-item"
                      value={item.label}
                      keywords={item.keywords ? [...item.keywords] : undefined}
                      onSelect={() => select(item)}
                    >
                      <span class="item-label">{item.label}</span>
                      {#if item.shortcut}<kbd class="shortcut">{item.shortcut}</kbd>{/if}
                    </Command.Item>
                  {/each}
                </Command.GroupItems>
              </Command.Group>
            {/each}
          </Command.Viewport>
        </Command.List>
      </Command.Root>
    </Dialog.Content>
  </Dialog.Portal>
</Dialog.Root>

<style>
  :global(.alfa-palette-overlay) {
    position: fixed;
    inset: 0;
    z-index: 100;
    background: var(--alfa-color-scrim);
  }
  :global(.alfa-palette) {
    position: fixed;
    z-index: 101;
    top: 15vh;
    left: 50%;
    width: min(640px, calc(100vw - 32px));
    max-height: 60vh;
    display: flex;
    flex-direction: column;
    transform: translateX(-50%);
    border: 1px solid var(--alfa-color-border);
    border-radius: var(--alfa-radius-overlay);
    background: color-mix(in srgb, var(--alfa-color-surface) 86%, transparent);
    /* Jedyne dozwolone użycie w repo (nakładka Ctrl+K). */
    backdrop-filter: blur(24px) saturate(1.4);
    box-shadow: var(--alfa-shadow-3);
    overflow: hidden;
  }
  @media (forced-colors: active), (prefers-reduced-transparency: reduce) {
    :global(.alfa-palette) {
      background: var(--alfa-color-surface);
      backdrop-filter: none;
    }
  }
  :global(.alfa-palette-cmd) {
    display: flex;
    flex-direction: column;
    min-height: 0;
  }
  .input-row {
    display: flex;
    align-items: center;
    gap: var(--alfa-space-2);
    padding: var(--alfa-space-2) var(--alfa-space-3);
    border-bottom: 1px solid var(--alfa-color-border);
    color: var(--alfa-color-text-muted);
  }
  :global(.alfa-palette-input) {
    flex: 1;
    min-width: 0;
    height: 32px;
    border: 0;
    background: transparent;
    color: var(--alfa-color-text);
    font-size: var(--alfa-font-size-lg);
  }
  :global(.alfa-palette-input:focus) {
    outline: none;
  }
  :global(.alfa-palette-list) {
    overflow: auto;
    padding: var(--alfa-space-1);
  }
  :global(.alfa-palette-empty) {
    padding: var(--alfa-space-4);
    color: var(--alfa-color-text-muted);
    text-align: center;
    font-size: var(--alfa-font-size-sm);
  }
  :global(.alfa-palette-heading) {
    padding: var(--alfa-space-2) var(--alfa-space-2) var(--alfa-space-1);
    color: var(--alfa-color-text-subtle);
    font-size: var(--alfa-font-size-xs);
    font-weight: var(--alfa-weight-semibold);
    letter-spacing: 0.06em;
    text-transform: uppercase;
  }
  :global(.alfa-palette-item) {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--alfa-space-3);
    min-height: 32px;
    padding: 0 var(--alfa-space-2);
    border-radius: var(--alfa-radius-control);
    font-size: var(--alfa-font-size-md);
  }
  :global(.alfa-palette-item[data-selected]) {
    background: var(--alfa-color-surface3);
  }
  kbd {
    padding: 1px 6px;
    border: 1px solid var(--alfa-color-border);
    border-radius: 4px;
    background: var(--alfa-color-surface2);
    color: var(--alfa-color-text-muted);
    font-size: var(--alfa-font-size-xs);
  }
</style>
