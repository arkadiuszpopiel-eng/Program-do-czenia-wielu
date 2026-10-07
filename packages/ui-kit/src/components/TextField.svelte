<script lang="ts">
  import type { HTMLInputAttributes } from 'svelte/elements';

  interface Props extends Omit<HTMLInputAttributes, 'value'> {
    value?: string | number;
    /** Widoczna etykieta (zalecana). */
    label?: string;
    hint?: string;
    error?: string;
    input?: HTMLInputElement | null;
  }

  let {
    value = $bindable(''),
    label,
    hint,
    error,
    input = $bindable(null),
    id,
    class: className = '',
    ...rest
  }: Props = $props();

  const autoId = $props.id();
  const fieldId = $derived(id ?? `${autoId}-field`);
  const hintId = $derived(`${fieldId}-hint`);
</script>

<div class="field {className}">
  {#if label}<label class="label" for={fieldId}>{label}</label>{/if}
  <input
    {...rest}
    id={fieldId}
    bind:this={input}
    bind:value
    aria-invalid={error ? true : undefined}
    aria-describedby={hint || error ? hintId : undefined}
  />
  {#if error}
    <span class="hint error" id={hintId} role="alert">{error}</span>
  {:else if hint}
    <span class="hint" id={hintId}>{hint}</span>
  {/if}
</div>

<style>
  .field {
    display: flex;
    flex-direction: column;
    gap: var(--alfa-space-1);
    min-width: 0;
  }
  .label {
    font-size: var(--alfa-font-size-sm);
    font-weight: var(--alfa-weight-semibold);
  }
  input {
    height: var(--alfa-size-control);
    padding: 0 var(--alfa-space-3);
    border: 1px solid var(--alfa-color-border);
    border-radius: var(--alfa-radius-control);
    background: var(--alfa-color-surface);
    color: var(--alfa-color-text);
    font-size: var(--alfa-font-size-md);
    min-width: 0;
  }
  input:hover:not(:disabled) {
    border-color: var(--alfa-color-border-strong);
  }
  input[aria-invalid='true'] {
    border-color: var(--alfa-color-error);
  }
  input::placeholder {
    color: var(--alfa-color-text-subtle);
  }
  .hint {
    color: var(--alfa-color-text-muted);
    font-size: var(--alfa-font-size-xs);
  }
  .error {
    color: var(--alfa-color-error);
  }
</style>
