<!--
  Lista wyboru w stylu Alfy (wzorzec WAI-ARIA „select-only combobox"): przycisk + własna lista opcji.
  Nie używamy natywnego <select> — jego listę rysuje system, poza stylami i motywami interfejsu
  (w stylu „Szkło" opcje były nieczytelne). Klawiatura: strzałki / Home / End, Enter i spacja
  wybierają, Esc zamyka, litery przeskakują do opcji; fokus zostaje na przycisku.
-->
<script lang="ts">
  import Check from '@lucide/svelte/icons/check';
  import ChevronDown from '@lucide/svelte/icons/chevron-down';
  import { tick } from 'svelte';

  interface Option {
    readonly value: string;
    readonly label: string;
    readonly disabled?: boolean;
  }

  interface Props {
    value?: string;
    options: readonly Option[];
    id?: string;
    label?: string;
    labelledby?: string;
    describedby?: string;
    disabled?: boolean;
    size?: 'sm' | 'md';
    onchange?: (value: string) => void;
  }

  let {
    value = $bindable(''),
    options,
    id,
    label,
    labelledby,
    describedby,
    disabled = false,
    size = 'md',
    onchange,
  }: Props = $props();

  const listId = $props.id();
  let anchor = $state<HTMLElement | null>(null);
  let open = $state(false);
  let upward = $state(false);
  let active = $state(-1);
  let typed = '';
  let typedAt = 0;

  const current = $derived(options.find((o) => o.value === value));
  const optionId = (index: number) => `${listId}-o${index}`;

  /** Najbliższa włączona opcja od `from` w kierunku `step`; -1, gdy żadna. */
  function enabledFrom(from: number, step: 1 | -1): number {
    for (let i = from; i >= 0 && i < options.length; i += step) {
      if (!options[i]?.disabled) return i;
    }
    return -1;
  }

  async function show(start?: number) {
    if (disabled) return;
    if (anchor) upward = anchor.getBoundingClientRect().bottom > window.innerHeight * 0.6;
    const selected = options.findIndex((o) => o.value === value && !o.disabled);
    active = start ?? (selected >= 0 ? selected : enabledFrom(0, 1));
    open = true;
    await tick();
    reveal();
  }

  function reveal() {
    if (active >= 0)
      document.getElementById(optionId(active))?.scrollIntoView({ block: 'nearest' });
  }

  function move(to: number) {
    if (to < 0) return;
    active = to;
    reveal();
  }

  function choose(index: number) {
    const option = options[index];
    open = false;
    if (!option || option.disabled || option.value === value) return;
    value = option.value;
    onchange?.(value);
  }

  /** Litery: przeskok do pierwszej opcji zaczynającej się od wpisanego ciągu. */
  function typeahead(char: string): number {
    const now = Date.now();
    typed = now - typedAt > 700 ? char : typed + char;
    typedAt = now;
    const needle = typed.toLocaleLowerCase();
    return options.findIndex((o) => !o.disabled && o.label.toLocaleLowerCase().startsWith(needle));
  }

  function onKey(event: KeyboardEvent) {
    const { key } = event;
    if (!open) {
      if (key === 'ArrowDown' || key === 'ArrowUp' || key === 'Enter' || key === ' ') {
        event.preventDefault();
        void show();
      } else if (key === 'Home' || key === 'End') {
        event.preventDefault();
        void show(key === 'Home' ? enabledFrom(0, 1) : enabledFrom(options.length - 1, -1));
      } else if (key.length === 1 && !event.ctrlKey && !event.altKey && !event.metaKey) {
        const hit = typeahead(key);
        if (hit >= 0) void show(hit);
      }
      return;
    }
    if (key === 'Escape') {
      event.preventDefault();
      event.stopPropagation();
      open = false;
    } else if (key === 'Tab') {
      open = false;
    } else if (key === 'Enter' || key === ' ') {
      event.preventDefault();
      choose(active);
    } else if (key === 'ArrowDown') {
      event.preventDefault();
      move(enabledFrom(active + 1, 1));
    } else if (key === 'ArrowUp') {
      event.preventDefault();
      move(enabledFrom(active - 1, -1));
    } else if (key === 'Home') {
      event.preventDefault();
      move(enabledFrom(0, 1));
    } else if (key === 'End') {
      event.preventDefault();
      move(enabledFrom(options.length - 1, -1));
    } else if (key.length === 1 && !event.ctrlKey && !event.altKey && !event.metaKey) {
      move(typeahead(key));
    }
  }

  function onWindowPointer(event: PointerEvent) {
    if (open && anchor && !anchor.contains(event.target as Node)) open = false;
  }
</script>

<svelte:window onpointerdown={onWindowPointer} />

<span class="select {size}" bind:this={anchor}>
  <button
    type="button"
    class="trigger"
    role="combobox"
    {id}
    {disabled}
    aria-haspopup="listbox"
    aria-expanded={open}
    aria-controls={listId}
    aria-activedescendant={open && active >= 0 ? optionId(active) : undefined}
    aria-label={labelledby ? undefined : label}
    aria-labelledby={labelledby}
    aria-describedby={describedby}
    data-value={value}
    onclick={() => (open ? (open = false) : void show())}
    onkeydown={onKey}
  >
    <span class="value">{current?.label ?? ''}</span>
    <ChevronDown size={14} strokeWidth={1.5} aria-hidden="true" class="chev" />
  </button>
  {#if open}
    <div
      class="list"
      class:upward
      role="listbox"
      id={listId}
      aria-label={labelledby ? undefined : label}
      aria-labelledby={labelledby}
    >
      {#each options as option, index (option.value)}
        <!-- Fokus zostaje na przycisku (aria-activedescendant), dlatego opcja nie obsługuje klawiszy. -->
        <div
          class="option"
          class:active={index === active}
          role="option"
          id={optionId(index)}
          tabindex="-1"
          aria-selected={option.value === value}
          aria-disabled={option.disabled ? true : undefined}
          data-value={option.value}
          onpointerdown={(event) => event.preventDefault()}
          onpointermove={() => {
            if (!option.disabled) active = index;
          }}
          onclick={(event) => {
            // W <label> klik opcji trafiłby jeszcze raz do przycisku i otworzył listę ponownie.
            event.preventDefault();
            choose(index);
          }}
          onkeydown={onKey}
        >
          <span class="text">{option.label}</span>
          {#if option.value === value}
            <Check size={14} strokeWidth={2} aria-hidden="true" />
          {/if}
        </div>
      {/each}
    </div>
  {/if}
</span>

<style>
  .select {
    position: relative;
    display: inline-flex;
    align-items: center;
    min-width: 0;
    max-width: 100%;
  }
  .trigger {
    display: inline-flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--alfa-space-2);
    width: 100%;
    min-width: 140px;
    max-width: 100%;
    height: var(--alfa-size-control);
    padding: 0 var(--alfa-space-2) 0 var(--alfa-space-3);
    border: 1px solid var(--alfa-color-border);
    border-radius: var(--alfa-radius-control);
    background: var(--alfa-color-surface);
    color: var(--alfa-color-text);
    font: inherit;
    font-size: var(--alfa-font-size-md);
    text-align: start;
    cursor: pointer;
  }
  .sm .trigger {
    height: 28px;
    min-width: 0;
    font-size: var(--alfa-font-size-sm);
  }
  .trigger:hover:not(:disabled),
  .trigger[aria-expanded='true'] {
    border-color: var(--alfa-color-border-strong);
  }
  .trigger:disabled {
    opacity: 0.5;
    cursor: default;
  }
  .trigger:focus-visible {
    outline: var(--alfa-size-focus-ring) solid var(--alfa-color-focus);
    outline-offset: 1px;
  }
  .value,
  .text {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .trigger :global(.chev) {
    flex: none;
    color: var(--alfa-color-text-muted);
  }
  .list {
    position: absolute;
    top: calc(100% + 4px);
    left: 0;
    z-index: 60;
    display: flex;
    flex-direction: column;
    min-width: 100%;
    width: max-content;
    max-width: min(360px, calc(var(--alfa-vw, 100vw) - 16px));
    max-height: min(320px, calc(var(--alfa-vh, 100vh) * 0.5));
    overflow-y: auto;
    padding: var(--alfa-space-1);
    border: 1px solid var(--alfa-color-border);
    border-radius: var(--alfa-radius-card);
    background: var(--alfa-color-surface);
    box-shadow: var(--alfa-shadow-3);
    color: var(--alfa-color-text);
    font-size: var(--alfa-font-size-md);
  }
  .sm .list {
    font-size: var(--alfa-font-size-sm);
  }
  .list.upward {
    top: auto;
    bottom: calc(100% + 4px);
  }
  .option {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--alfa-space-3);
    min-height: 30px;
    padding: 0 var(--alfa-space-2) 0 var(--alfa-space-3);
    border-radius: calc(var(--alfa-radius-control) - 2px);
    cursor: pointer;
  }
  .option.active {
    background: var(--alfa-color-surface2);
  }
  .option[aria-selected='true'] {
    font-weight: 600;
  }
  .option[aria-disabled='true'] {
    color: var(--alfa-color-text-subtle);
    cursor: default;
  }
  @media (forced-colors: active) {
    .option.active {
      outline: 2px solid Highlight;
      outline-offset: -2px;
    }
  }
</style>
