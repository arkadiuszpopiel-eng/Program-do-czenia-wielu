<!--
  Wskaźnik kamery w pasku tytułu: widoczny zawsze, gdy którakolwiek kamera jest otwarta, otwiera
  się albo jeszcze się zamyka (nie da się
  go ukryć ustawieniem — projekt ADR 0025). Pokazuje liczbę działających kamer; klik prowadzi do
  Ustawień → Kamery.
-->
<script lang="ts">
  import Camera from '@lucide/svelte/icons/camera';
  import { useApp } from '../../state/context';

  const app = useApp();
  const { t } = app.i18n;
  // Kamera otwierana i zamykana też może już (albo jeszcze) patrzeć — znak świeci w każdym
  // z tych stanów, nie tylko przy „działa”.
  const OPEN = ['live', 'opening', 'closing'];
  const live = $derived((app.cameras?.cameras ?? []).filter((c) => OPEN.includes(c.state)));
  const names = $derived(live.map((c) => c.name).join(', '));
</script>

{#if live.length > 0}
  <button
    type="button"
    class="cam"
    aria-label={t('cam.indicator', { n: live.length, names })}
    title={t('cam.indicator', { n: live.length, names })}
    onclick={() => app.openSettings('cameras')}
  >
    <Camera size={15} strokeWidth={1.75} aria-hidden="true" />
    <span class="dot" aria-hidden="true"></span>
    {#if live.length > 1}<span class="count" aria-hidden="true">{live.length}</span>{/if}
  </button>
{/if}

<style>
  .cam {
    position: relative;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 5px;
    min-width: 34px;
    height: 34px;
    padding: 0 var(--alfa-space-2);
    border: 1px solid var(--alfa-color-error);
    border-radius: var(--alfa-radius-full);
    background: var(--alfa-color-bg);
    color: var(--alfa-color-text);
  }
  .cam:hover {
    background: var(--alfa-color-surface2);
  }
  .dot {
    width: 8px;
    height: 8px;
    border-radius: var(--alfa-radius-full);
    background: var(--alfa-color-error);
  }
  .count {
    font-size: var(--alfa-font-size-xs);
    font-variant-numeric: tabular-nums;
    font-weight: var(--alfa-weight-semibold);
  }
</style>
