<script lang="ts">
  import type { HTMLButtonAttributes } from 'svelte/elements';
  import type { Snippet } from 'svelte';
  import type { AgentId } from '../tokens';

  type Variant = 'primary' | 'secondary' | 'ghost' | 'danger';
  type Size = 'sm' | 'md';

  interface Props extends HTMLButtonAttributes {
    variant?: Variant;
    size?: Size;
    /** Kolor akcentu agentki dla wariantu primary/ghost. */
    agent?: AgentId;
    loading?: boolean;
    icon?: Snippet;
    children?: Snippet;
  }

  let {
    variant = 'secondary',
    size = 'md',
    agent,
    loading = false,
    disabled = false,
    icon,
    children,
    class: className = '',
    type = 'button',
    ...rest
  }: Props = $props();
</script>

<button
  {...rest}
  {type}
  class="btn {variant} {size} {className}"
  style:--accent={agent ? `var(--alfa-agent-${agent})` : undefined}
  disabled={disabled || loading}
  aria-busy={loading || undefined}
>
  {#if loading}
    <span class="spinner" aria-hidden="true"></span>
  {:else if icon}
    <span class="icon" aria-hidden="true">{@render icon()}</span>
  {/if}
  {#if children}
    <span class="label">{@render children()}</span>
  {/if}
</button>

<style>
  .btn {
    --accent: var(--alfa-color-text);
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: var(--alfa-space-2);
    min-height: var(--alfa-size-control);
    padding: 0 var(--alfa-space-3);
    border: 1px solid transparent;
    border-radius: var(--alfa-radius-control);
    font-weight: var(--alfa-weight-semibold);
    font-size: var(--alfa-font-size-md);
    line-height: 1;
    white-space: nowrap;
    background: transparent;
    transition:
      transform var(--alfa-duration-fast) var(--alfa-ease-out),
      opacity var(--alfa-duration-fast) var(--alfa-ease-out);
  }
  .btn:active:not(:disabled) {
    transform: scale(0.98);
  }
  .btn:disabled {
    opacity: 0.5;
  }
  .sm {
    min-height: 28px;
    padding: 0 var(--alfa-space-2);
    font-size: var(--alfa-font-size-sm);
  }
  .primary {
    background: var(--accent);
    color: var(--alfa-color-text-on-accent);
    border-color: transparent;
  }
  .primary:hover:not(:disabled) {
    opacity: 0.9;
  }
  .secondary {
    background: var(--alfa-color-surface);
    border-color: var(--alfa-color-border);
    box-shadow: var(--alfa-shadow-1);
  }
  .secondary:hover:not(:disabled) {
    background: var(--alfa-color-surface2);
  }
  .ghost {
    color: var(--accent);
  }
  .ghost:hover:not(:disabled) {
    background: var(--alfa-color-surface2);
  }
  .danger {
    background: var(--alfa-color-error);
    color: var(--alfa-color-text-on-accent);
  }
  .icon {
    display: inline-flex;
  }
  .spinner {
    width: 14px;
    height: 14px;
    border: 2px solid currentColor;
    border-right-color: transparent;
    border-radius: var(--alfa-radius-full);
    animation: spin 0.8s linear infinite;
  }
  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }
  @media (forced-colors: active) {
    .btn {
      border-color: ButtonText;
      forced-color-adjust: none;
      background: ButtonFace;
      color: ButtonText;
    }
    .primary,
    .danger {
      background: Highlight;
      color: HighlightText;
    }
  }
</style>
