<!--
  Wprowadzenie → karty wyboru (radiogroup): znaczek, nazwa, opis i opcjonalna etykieta („zalecany”).
  Strzałki przenoszą wybór jak w `SegmentedControl`; zaznaczoną kartę wyróżnia ramka i znak wyboru.
-->
<script lang="ts">
  interface Option {
    readonly value: string;
    readonly badge: string;
    readonly title: string;
    readonly description: string;
    readonly tag?: string | null;
  }

  interface Props {
    value?: string;
    options: readonly Option[];
    label: string;
    columns?: 1 | 2;
  }

  let { value = $bindable(''), options, label, columns = 1 }: Props = $props();
  const groupId = $props.id();

  function key(event: KeyboardEvent, index: number) {
    const n = options.length;
    let next = -1;
    if (event.key === 'ArrowRight' || event.key === 'ArrowDown') next = (index + 1) % n;
    if (event.key === 'ArrowLeft' || event.key === 'ArrowUp') next = (index - 1 + n) % n;
    const option = options[next];
    if (!option) return;
    event.preventDefault();
    value = option.value;
    const group = (event.currentTarget as HTMLElement).parentElement;
    group?.querySelectorAll<HTMLElement>('[role="radio"]')[next]?.focus();
  }
</script>

<div class="cards" class:two={columns === 2} role="radiogroup" aria-label={label}>
  {#each options as option, i (option.value)}
    <button
      type="button"
      role="radio"
      class="card"
      aria-checked={value === option.value}
      aria-describedby="{groupId}-{i}"
      tabindex={value === option.value || (!options.some((o) => o.value === value) && i === 0)
        ? 0
        : -1}
      onclick={() => (value = option.value)}
      onkeydown={(e) => key(e, i)}
    >
      <span class="badge" aria-hidden="true">{option.badge}</span>
      <span class="text">
        <span class="title"
          >{option.title}{#if option.tag}<span class="tag">{option.tag}</span>{/if}</span
        >
        <span class="desc" id="{groupId}-{i}">{option.description}</span>
      </span>
      <span class="check" aria-hidden="true">
        <svg
          width="12"
          height="12"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          stroke-width="3.5"
          stroke-linecap="round"
          stroke-linejoin="round"><path d="M5 12l5 5 9-10" /></svg
        >
      </span>
    </button>
  {/each}
</div>

<style>
  .cards {
    display: grid;
    grid-template-columns: 1fr;
    gap: var(--alfa-space-2);
  }
  .cards.two {
    grid-template-columns: repeat(2, minmax(0, 1fr));
  }
  .card {
    display: flex;
    align-items: flex-start;
    gap: var(--alfa-space-3);
    padding: var(--alfa-space-3);
    border: 1px solid var(--alfa-color-border);
    border-radius: var(--alfa-radius-card);
    background: var(--alfa-color-surface);
    color: var(--alfa-color-text);
    text-align: left;
  }
  .card:hover {
    border-color: var(--alfa-color-border-strong);
    background: var(--alfa-color-surface2);
  }
  .badge {
    display: inline-flex;
    flex: none;
    align-items: center;
    justify-content: center;
    min-width: 36px;
    height: 36px;
    padding: 0 var(--alfa-space-1);
    border-radius: var(--alfa-radius-control);
    background: var(--alfa-color-surface3);
    font-size: var(--alfa-font-size-md);
    font-weight: var(--alfa-weight-semibold);
    font-variant-numeric: tabular-nums;
  }
  .text {
    display: flex;
    flex: 1;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }
  .title {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--alfa-space-2);
    font-size: var(--alfa-font-size-md);
    font-weight: var(--alfa-weight-semibold);
  }
  .tag {
    padding: 1px var(--alfa-space-2);
    border: 1px solid var(--alfa-color-success);
    border-radius: var(--alfa-radius-full);
    color: var(--alfa-color-success);
    font-size: var(--alfa-font-size-xs);
    font-weight: var(--alfa-weight-semibold);
  }
  .desc {
    color: var(--alfa-color-text-muted);
    font-size: var(--alfa-font-size-sm);
    font-weight: var(--alfa-weight-regular);
    line-height: 1.45;
  }
  .check {
    display: inline-flex;
    flex: none;
    align-items: center;
    justify-content: center;
    width: 20px;
    height: 20px;
    border: 1px solid var(--alfa-color-border-strong);
    border-radius: var(--alfa-radius-full);
    color: transparent;
  }
  .card[aria-checked='true'] {
    border-color: var(--alfa-color-text);
    background: var(--alfa-color-surface2);
    box-shadow: inset 0 0 0 1px var(--alfa-color-text);
  }
  .card[aria-checked='true'] .badge,
  .card[aria-checked='true'] .check {
    border-color: var(--alfa-color-text);
    background: var(--alfa-color-text);
    color: var(--alfa-color-text-on-accent);
  }
  @media (max-width: 760px) {
    .cards.two {
      grid-template-columns: 1fr;
    }
  }
</style>
