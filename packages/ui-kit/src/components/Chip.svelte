<script lang="ts">
  import type { Snippet } from 'svelte';
  import type { AgentId, SemanticColor } from '../tokens';

  interface Props {
    /** Akcent agentki (pierścień/tekst w jej kolorze). */
    agent?: AgentId;
    tone?: SemanticColor;
    size?: 'sm' | 'md';
    /** Chip klikalny (np. wybór agentki w composerze) — renderuje <button>. */
    onclick?: (event: MouseEvent) => void;
    selected?: boolean;
    disabled?: boolean;
    /** Etykieta dostępności, gdy treść to np. sama ikona. */
    label?: string;
    icon?: Snippet;
    children: Snippet;
  }

  let {
    agent,
    tone,
    size = 'md',
    onclick,
    selected = false,
    disabled = false,
    label,
    icon,
    children,
  }: Props = $props();

  const accent = $derived(
    agent ? `var(--alfa-agent-${agent})` : tone ? `var(--alfa-color-${tone})` : undefined,
  );
</script>

{#if onclick}
  <button
    type="button"
    class="chip {size} interactive"
    class:selected
    class:accented={Boolean(accent)}
    style:--accent={accent}
    aria-pressed={selected}
    aria-label={label}
    {disabled}
    {onclick}
  >
    {#if icon}<span class="icon" aria-hidden="true">{@render icon()}</span>{/if}
    {@render children()}
  </button>
{:else}
  <span
    class="chip {size}"
    class:accented={Boolean(accent)}
    style:--accent={accent}
    aria-label={label}
  >
    {#if icon}<span class="icon" aria-hidden="true">{@render icon()}</span>{/if}
    {@render children()}
  </span>
{/if}

<style>
  .chip {
    --accent: var(--alfa-color-text-muted);
    display: inline-flex;
    align-items: center;
    gap: var(--alfa-space-1);
    height: var(--alfa-size-hit-target);
    padding: 0 var(--alfa-space-2);
    border: 1px solid var(--alfa-color-border);
    border-radius: var(--alfa-radius-full);
    background: var(--alfa-color-surface);
    color: var(--alfa-color-text-muted);
    font-size: var(--alfa-font-size-xs);
    font-weight: var(--alfa-weight-semibold);
    line-height: 1;
    white-space: nowrap;
  }
  .md {
    height: 28px;
    padding: 0 var(--alfa-space-3);
    font-size: var(--alfa-font-size-sm);
  }
  .accented {
    color: var(--accent);
    border-color: color-mix(in srgb, var(--accent) 45%, transparent);
  }
  .interactive {
    transition: transform var(--alfa-duration-fast) var(--alfa-ease-out);
  }
  .interactive:hover:not(:disabled) {
    background: var(--alfa-color-surface2);
  }
  .interactive:active:not(:disabled) {
    transform: scale(0.97);
  }
  .selected {
    background: var(--alfa-color-surface3);
    border-color: var(--accent);
    color: var(--alfa-color-text);
  }
  .interactive:disabled {
    opacity: 0.5;
  }
  .icon {
    display: inline-flex;
  }
</style>
