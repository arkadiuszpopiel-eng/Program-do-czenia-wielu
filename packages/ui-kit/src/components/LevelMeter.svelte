<!-- Wskaźnik głośności (≤ 30 kl./s po stronie źródła); ruch tylko przez transform. -->
<script lang="ts">
  interface Props {
    level: number;
    label: string;
    accent?: string;
  }

  let { level, label, accent = 'var(--alfa-color-success)' }: Props = $props();
  const value = $derived(Math.max(0, Math.min(1, level)));
</script>

<div
  class="meter"
  role="meter"
  aria-label={label}
  aria-valuemin={0}
  aria-valuemax={100}
  aria-valuenow={Math.round(value * 100)}
  style:--accent={accent}
>
  <span class="fill" style:transform="scaleX({value})"></span>
</div>

<style>
  .meter {
    position: relative;
    width: 100%;
    height: 8px;
    overflow: hidden;
    border-radius: var(--alfa-radius-full);
    background: var(--alfa-color-surface3);
  }
  .fill {
    position: absolute;
    inset: 0;
    background: var(--accent);
    transform-origin: left;
    transition: transform 60ms linear;
  }
</style>
