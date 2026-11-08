<!-- Grupa opcji (radiogroup) z nawigacją strzałkami — np. motyw, tryb importu, zakres. -->
<script lang="ts">
  interface Option {
    readonly value: string;
    readonly label: string;
    readonly description?: string;
  }

  interface Props {
    value?: string;
    options: readonly Option[];
    label: string;
    /** `cards` — pionowe karty z opisem (np. poziomy autonomii). */
    variant?: 'inline' | 'cards';
    onchange?: (value: string) => void;
  }

  let { value = $bindable(''), options, label, variant = 'inline', onchange }: Props = $props();
  const groupId = $props.id();

  function pick(next: string) {
    value = next;
    onchange?.(next);
  }

  function key(event: KeyboardEvent, index: number) {
    const n = options.length;
    let next = -1;
    if (event.key === 'ArrowRight' || event.key === 'ArrowDown') next = (index + 1) % n;
    if (event.key === 'ArrowLeft' || event.key === 'ArrowUp') next = (index - 1 + n) % n;
    if (next < 0) return;
    event.preventDefault();
    const option = options[next];
    if (!option) return;
    pick(option.value);
    const group = (event.currentTarget as HTMLElement).parentElement;
    group?.querySelectorAll<HTMLElement>('[role="radio"]')[next]?.focus();
  }
</script>

<div class="group {variant}" role="radiogroup" aria-label={label}>
  {#each options as option, i (option.value)}
    <button
      type="button"
      role="radio"
      class="option"
      aria-checked={value === option.value}
      aria-describedby={option.description ? `${groupId}-${i}` : undefined}
      tabindex={value === option.value || (!options.some((o) => o.value === value) && i === 0)
        ? 0
        : -1}
      onclick={() => pick(option.value)}
      onkeydown={(e) => key(e, i)}
    >
      <span class="label">{option.label}</span>
      {#if option.description}
        <span class="desc" id="{groupId}-{i}">{option.description}</span>
      {/if}
    </button>
  {/each}
</div>

<style>
  .group {
    display: inline-flex;
    flex-wrap: wrap;
    gap: 2px;
    padding: 2px;
    border: 1px solid var(--alfa-color-border);
    border-radius: var(--alfa-radius-control);
    background: var(--alfa-color-surface2);
  }
  .option {
    min-height: 28px;
    padding: 0 var(--alfa-space-3);
    border: 0;
    border-radius: 4px;
    background: transparent;
    color: var(--alfa-color-text-muted);
    font-size: var(--alfa-font-size-sm);
    font-weight: var(--alfa-weight-semibold);
  }
  .option[aria-checked='true'] {
    background: var(--alfa-color-surface);
    color: var(--alfa-color-text);
    box-shadow: var(--alfa-shadow-1);
  }
  .cards {
    display: flex;
    flex-direction: column;
    gap: var(--alfa-space-2);
    padding: 0;
    border: 0;
    background: transparent;
  }
  .cards .option {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 2px;
    padding: var(--alfa-space-2) var(--alfa-space-3);
    border: 1px solid var(--alfa-color-border);
    border-radius: var(--alfa-radius-card);
    background: var(--alfa-color-surface);
    text-align: left;
  }
  .cards .option[aria-checked='true'] {
    border-color: var(--alfa-color-text);
    box-shadow: inset 0 0 0 1px var(--alfa-color-text);
  }
  .cards .label {
    color: var(--alfa-color-text);
  }
  .desc {
    color: var(--alfa-color-text-muted);
    font-size: var(--alfa-font-size-xs);
    font-weight: var(--alfa-weight-regular);
  }
</style>
