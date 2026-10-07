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
  input {
    width: 16px;
    height: 16px;
    margin: 3px 0 0;
    accent-color: var(--alfa-color-info);
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
