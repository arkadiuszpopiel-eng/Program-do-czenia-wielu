<script lang="ts">
  interface Props {
    checked?: boolean;
    label: string;
    description?: string;
    disabled?: boolean;
    onchange?: (checked: boolean) => void;
  }

  let {
    checked = $bindable(false),
    label,
    description,
    disabled = false,
    onchange,
  }: Props = $props();
  const id = $props.id();
</script>

<label class="check" class:disabled>
  <input
    type="checkbox"
    bind:checked
    {disabled}
    aria-describedby={description ? `${id}-d` : undefined}
    onchange={() => onchange?.(checked)}
  />
  <span class="text">
    <span class="label">{label}</span>
    {#if description}<span class="desc" id="{id}-d">{description}</span>{/if}
  </span>
</label>

<style>
  .check {
    display: flex;
    align-items: flex-start;
    gap: var(--alfa-space-2);
    min-height: var(--alfa-size-hit-target);
    font-size: var(--alfa-font-size-sm);
  }
  /* Własny wygląd zamiast systemowego pola: ramka z tokenów, znaczek rysowany maską. */
  input {
    appearance: none;
    flex: none;
    display: grid;
    place-content: center;
    width: 16px;
    height: 16px;
    margin: 3px 0 0;
    border: 1px solid var(--alfa-color-border-strong);
    border-radius: 4px;
    background: var(--alfa-color-surface);
    cursor: pointer;
  }
  input::before {
    content: '';
    width: 10px;
    height: 10px;
    background: var(--alfa-color-text-on-accent, var(--alfa-color-bg));
    clip-path: polygon(14% 52%, 0 66%, 38% 100%, 100% 22%, 86% 10%, 37% 70%);
    transform: scale(0);
  }
  input:checked {
    border-color: var(--alfa-color-accent, var(--alfa-color-info));
    background: var(--alfa-color-accent, var(--alfa-color-info));
  }
  input:checked::before {
    transform: scale(1);
  }
  input:disabled {
    cursor: default;
  }
  @media (forced-colors: active) {
    input {
      appearance: auto;
    }
    input::before {
      display: none;
    }
  }
  .text {
    display: flex;
    flex-direction: column;
  }
  .desc {
    color: var(--alfa-color-text-muted);
    font-size: var(--alfa-font-size-xs);
  }
  .disabled {
    opacity: 0.55;
  }
</style>
