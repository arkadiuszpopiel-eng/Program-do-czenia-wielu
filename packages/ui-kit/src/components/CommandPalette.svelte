<!--
  Paleta poleceń (Ctrl+K). UWAGA: to JEDYNY plik w repo, w którym dozwolony jest
  `backdrop-filter` (PLAN.md §14.3 / §14.7). <dialog> jest stale zamontowany, a lista bits-ui Command
  renderuje się raz — otwarcie to `show()` (~5 ms), co mieści się w budżecie ≤ 50 ms (§14.7).
  `showModal()` kosztował ~30 ms (usztywnienie całego dokumentu jako `inert`), więc modalność
  zapewniamy sami: `aria-modal`, pułapka fokusu (Tab), Esc, tło zamykające kliknięciem.
-->
<script lang="ts">
  import { Command } from 'bits-ui';
  import Search from '@lucide/svelte/icons/search';
  import type { CommandItem } from '../types';

  interface Props {
    /** Czy paleta jest otwarta (bindable). */
    open?: boolean;
    items: readonly CommandItem[];
    placeholder?: string;
    /** Rejestruj globalny skrót Ctrl+K (domyślnie tak). */
    hotkey?: boolean;
    /**
     * Własne dopasowanie (0 = ukryj, 1 = idealne). Dostaje zapytanie oraz etykietę i słowa kluczowe
     * pozycji. Domyślnie — algorytm bits-ui.
     */
    filter?: (search: string, label: string, keywords: readonly string[]) => number;
    labels?: Partial<{ title: string; search: string; empty: string }>;
    /** Bieżące zapytanie (bindable) — np. by dołączać rzadkie pozycje dopiero przy wyszukiwaniu. */
    search?: string;
  }

  let {
    open = $bindable(false),
    items,
    placeholder = 'Wpisz polecenie…',
    hotkey = true,
    filter,
    labels = {},
    search = $bindable(''),
  }: Props = $props();

  const text = $derived({
    title: 'Paleta poleceń',
    search: 'Szukaj polecenia',
    empty: 'Brak poleceń pasujących do zapytania.',
    ...labels,
  });

  /** bits-ui przekazuje `value` (id) i `keywords` (etykieta jako pierwsze słowo kluczowe). */
  const bitsFilter = $derived(
    filter
      ? (_value: string, search: string, keywords: string[] = []) =>
          filter(search, keywords[0] ?? '', keywords.slice(1))
      : undefined,
  );

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

  let dialog = $state<HTMLDialogElement | null>(null);
  let returnFocus: HTMLElement | null = null;

  // Stan okna ustalamy synchronicznie w efekcie; zdarzenie `close` przychodzi asynchronicznie,
  // więc nie może nadpisać ponownego otwarcia (szybkie Esc → Ctrl+K).
  $effect(() => {
    const el = dialog;
    if (!el) return;
    if (open && !el.open) {
      const active = document.activeElement;
      returnFocus = active instanceof HTMLElement && !el.contains(active) ? active : null;
      el.show();
      el.querySelector<HTMLInputElement>('input')?.focus();
    } else if (!open && el.open) {
      el.close();
      restore();
    }
  });

  function restore() {
    search = '';
    // Fokus nie może zostać w ukrytym oknie (przeglądarka poprawia go dopiero przy renderowaniu).
    const active = document.activeElement;
    if (active instanceof HTMLElement && dialog?.contains(active)) active.blur();
    if (returnFocus?.isConnected) returnFocus.focus();
    returnFocus = null;
  }

  /** Zamknięcie spoza komponentu (np. przez przeglądarkę) — tylko gdy okno faktycznie zamknięte. */
  function onClose() {
    if (open && dialog && !dialog.open) {
      open = false;
      restore();
    }
  }

  function onKeydown(event: KeyboardEvent) {
    if (event.key === 'Escape') {
      event.preventDefault();
      event.stopPropagation();
      open = false;
      return;
    }
    if (event.key !== 'Tab' || !dialog) return;
    const focusable = Array.from(
      dialog.querySelectorAll<HTMLElement>('input, [tabindex]:not([tabindex="-1"])'),
    );
    const first = focusable[0];
    const last = focusable[focusable.length - 1];
    if (!first || !last) return;
    if (event.shiftKey && document.activeElement === first) {
      event.preventDefault();
      last.focus();
    } else if (!event.shiftKey && document.activeElement === last) {
      event.preventDefault();
      first.focus();
    }
  }

  function select(item: CommandItem) {
    open = false;
    item.onSelect?.();
  }
</script>

<svelte:window onkeydown={onWindowKeydown} />

<!-- Tło zamyka paletę kliknięciem; z klawiatury zamyka ją Esc (obsługa w oknie dialogu). -->
<div
  class="alfa-palette-scrim"
  hidden={!open}
  aria-hidden="true"
  onclick={() => (open = false)}
></div>
<dialog
  bind:this={dialog}
  class="alfa-palette"
  aria-label={text.title}
  aria-modal="true"
  onclose={onClose}
  onkeydown={onKeydown}
>
  <!-- vimBindings wyłączone: Ctrl+K/N/P/J to skróty aplikacji (m.in. zamknięcie palety Ctrl+K). -->
  <Command.Root
    class="alfa-palette-cmd"
    loop
    vimBindings={false}
    filter={bitsFilter}
    label={text.title}
  >
    <div class="input-row">
      <Search size={16} strokeWidth={1.5} aria-hidden="true" />
      <Command.Input
        class="alfa-palette-input"
        {placeholder}
        aria-label={text.search}
        bind:value={search}
      />
      <kbd class="esc">Esc</kbd>
    </div>
    <!-- tabindex: przewijana lista dostępna z klawiatury także poza polem wyszukiwania. -->
    <Command.List class="alfa-palette-list" tabindex={0}>
      <Command.Viewport>
        <Command.Empty class="alfa-palette-empty">{text.empty}</Command.Empty>
        {#each groups as [group, list] (group)}
          <Command.Group class="alfa-palette-group">
            <Command.GroupHeading class="alfa-palette-heading">{group}</Command.GroupHeading>
            <Command.GroupItems>
              {#each list as item (item.id)}
                <Command.Item
                  class="alfa-palette-item"
                  value={item.id}
                  keywords={[item.label, ...(item.keywords ?? [])]}
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
</dialog>

<style>
  .alfa-palette-scrim {
    position: fixed;
    inset: 0;
    z-index: 100;
    background: var(--alfa-color-scrim);
  }
  .alfa-palette-scrim[hidden] {
    display: none;
  }
  .alfa-palette {
    position: fixed;
    z-index: 101;
    inset: 15vh 0 auto 0;
    width: min(640px, calc(100vw - 32px));
    max-width: none;
    max-height: 60vh;
    margin: 0 auto;
    padding: 0;
    color: var(--alfa-color-text);
    border: 1px solid var(--alfa-color-border);
    border-radius: var(--alfa-radius-overlay);
    background: color-mix(in srgb, var(--alfa-color-surface) 86%, transparent);
    /* Jedyne dozwolone użycie w repo (nakładka Ctrl+K). */
    backdrop-filter: blur(24px) saturate(1.4);
    box-shadow: var(--alfa-shadow-3);
    overflow: hidden;
  }
  .alfa-palette[open] {
    display: flex;
    flex-direction: column;
  }
  @media (forced-colors: active), (prefers-reduced-transparency: reduce) {
    .alfa-palette {
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
