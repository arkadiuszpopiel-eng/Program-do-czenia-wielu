<script lang="ts">
  interface Props {
    checked?: boolean;
    /** Etykieta dostępności, gdy brak widocznej etykiety powiązanej przez `labelledby`. */
    label?: string;
    labelledby?: string;
    describedby?: string;
    id?: string;
    disabled?: boolean;
    onchange?: (checked: boolean) => void;
  }

  let {
    checked = $bindable(false),
    label,
    labelledby,
    describedby,
    id,
    disabled = false,
    onchange,
  }: Props = $props();

  function toggle() {
    checked = !checked;
    onchange?.(checked);
  }
</script>

<button
  type="button"
  role="switch"
  class="switch"
  {id}
  aria-checked={checked}
  aria-label={labelledby ? undefined : label}
  aria-labelledby={labelledby}
  aria-describedby={describedby}
  {disabled}
  onclick={toggle}
>
  <span class="track" aria-hidden="true"><span class="thumb"></span></span>
</button>

<style>
  .switch {
    display: inline-flex;
    align-items: center;
    flex: none;
    min-width: 40px;
    min-height: var(--alfa-size-hit-target);
    padding: 0;
    border: 0;
    background: transparent;
  }
  .track {
    position: relative;
    width: 40px;
    height: 20px;
    border: 1px solid var(--alfa-color-border-strong);
    border-radius: var(--alfa-radius-full);
    background: var(--alfa-color-surface2);
  }
  .thumb {
    position: absolute;
    top: 3px;
    left: 3px;
    width: 12px;
    height: 12px;
    border-radius: var(--alfa-radius-full);
    background: var(--alfa-color-text-muted);
    transition: transform var(--alfa-duration-fast) var(--alfa-ease-out);
  }
  .switch[aria-checked='true'] .track {
    border-color: var(--alfa-color-info);
    background: var(--alfa-color-info);
  }
  .switch[aria-checked='true'] .thumb {
    background: var(--alfa-color-text-on-accent);
    transform: translateX(20px);
  }
  .switch:disabled {
    opacity: 0.5;
  }
  @media (forced-colors: active) {
    .track {
      border-color: ButtonText;
    }
    .switch[aria-checked='true'] .track {
      background: Highlight;
    }
  }
</style>
