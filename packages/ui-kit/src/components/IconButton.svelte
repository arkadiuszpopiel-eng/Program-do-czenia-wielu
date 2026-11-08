<script lang="ts">
  import type { HTMLButtonAttributes } from 'svelte/elements';
  import type { Snippet } from 'svelte';

  interface Props extends HTMLButtonAttributes {
    /** Etykieta dostępności (wymagana — przycisk ma tylko ikonę). */
    label: string;
    size?: 'sm' | 'md' | 'lg';
    /** Przycisk-przełącznik: stan wciśnięty (aria-pressed). */
    pressed?: boolean;
    tone?: 'neutral' | 'danger';
    children: Snippet;
  }

  let {
    label,
    size = 'md',
    pressed,
    tone = 'neutral',
    children,
    class: className = '',
    type = 'button',
    ...rest
  }: Props = $props();
</script>

<button
  {...rest}
  {type}
  class="icon-btn {size} {tone} {className}"
  aria-label={label}
  title={label}
  aria-pressed={pressed}
>
  <span class="glyph" aria-hidden="true">{@render children()}</span>
</button>

<style>
  .icon-btn {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    flex: none;
    width: var(--alfa-size-control);
    height: var(--alfa-size-control);
    padding: 0;
    border: 1px solid transparent;
    border-radius: var(--alfa-radius-control);
    background: transparent;
    color: var(--alfa-color-text-muted);
    transition:
      transform var(--alfa-duration-fast) var(--alfa-ease-out),
      opacity var(--alfa-duration-fast) var(--alfa-ease-out);
  }
  .sm {
    width: 28px;
    height: 28px;
  }
  .lg {
    width: 40px;
    height: 40px;
  }
  .icon-btn:hover:not(:disabled) {
    background: var(--alfa-color-surface2);
    color: var(--alfa-color-text);
  }
  .icon-btn:active:not(:disabled) {
    transform: scale(0.94);
  }
  .icon-btn[aria-pressed='true'] {
    background: var(--alfa-color-surface3);
    color: var(--alfa-color-text);
  }
  .danger {
    color: var(--alfa-color-error);
  }
  .icon-btn:disabled {
    opacity: 0.45;
  }
  .glyph {
    display: inline-flex;
  }
  @media (forced-colors: active) {
    .icon-btn {
      border-color: ButtonText;
    }
  }
</style>
