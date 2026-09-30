<script lang="ts">
  import ChevronDown from '@lucide/svelte/icons/chevron-down';

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
</script>

<span class="select {size}">
  <select
    {id}
    bind:value
    aria-label={labelledby ? undefined : label}
    aria-labelledby={labelledby}
    aria-describedby={describedby}
    {disabled}
    onchange={() => onchange?.(value)}
  >
    {#each options as option (option.value)}
      <option value={option.value} disabled={option.disabled}>{option.label}</option>
    {/each}
  </select>
  <ChevronDown size={14} strokeWidth={1.5} aria-hidden="true" class="chev" />
</span>

<style>
  .select {
    position: relative;
    display: inline-flex;
    align-items: center;
    min-width: 0;
  }
  select {
    appearance: none;
    width: 100%;
    min-width: 140px;
    max-width: 100%;
    height: var(--alfa-size-control);
    padding: 0 28px 0 var(--alfa-space-3);
    border: 1px solid var(--alfa-color-border);
    border-radius: var(--alfa-radius-control);
    background: var(--alfa-color-surface);
    color: var(--alfa-color-text);
    font-size: var(--alfa-font-size-md);
  }
  .sm select {
    height: 28px;
    min-width: 0;
    font-size: var(--alfa-font-size-sm);
  }
  select:hover:not(:disabled) {
    border-color: var(--alfa-color-border-strong);
  }
  select:disabled {
    opacity: 0.5;
  }
  .select :global(.chev) {
    position: absolute;
    right: 8px;
    pointer-events: none;
    color: var(--alfa-color-text-muted);
  }
</style>
