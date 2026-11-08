<!--
  Wprowadzenie → pomiar sprzętu: kafle z wynikiem (procesor, pamięć, karta, klasa, silnik modelu)
  i lista wniosków. Błąd pomiaru daje „Ponów”, nigdy wieczne „Mierzę…”.
-->
<script lang="ts">
  import type { DeviceProfile } from '../../api/types-hub';
  import LoadFailed from '../../components/shell/LoadFailed.svelte';
  import { useApp } from '../../state/context';

  interface Props {
    profile: DeviceProfile | null;
    measuring: boolean;
    error: string | null;
    onretry: () => void;
  }

  let { profile, measuring, error, onretry }: Props = $props();
  const app = useApp();
  const { t } = app.i18n;

  /** Nazwa karty bez powtórzonego producenta („NVIDIA NVIDIA GeForce…” z prawdziwego pomiaru). */
  function gpuName(gpu: { vendor: string; model: string }): string {
    return gpu.model.toLowerCase().startsWith(gpu.vendor.toLowerCase())
      ? gpu.model
      : `${gpu.vendor} ${gpu.model}`;
  }

  const tiles = $derived(
    profile
      ? [
          { key: 'cpu', label: t('dev.cpu'), value: profile.machine.cpu.model },
          {
            key: 'gpu',
            label: t('dev.gpu'),
            value: profile.machine.gpus.map(gpuName).join(', ') || t('common.none'),
          },
          {
            key: 'ram',
            label: t('dev.ram'),
            value: app.i18n.bytes(profile.machine.ram_mb * 1024 * 1024),
          },
          {
            key: 'class',
            label: t('dev.hwClass'),
            value: t(`dev.hw.${profile.recommendation.hw_class}`),
          },
          { key: 'llm', label: t('dev.llm'), value: profile.recommendation.llm_backend },
        ]
      : [],
  );
</script>

{#if error && !measuring}
  <LoadFailed {error} {onretry} />
{:else if measuring || !profile}
  <p class="busy" role="status">
    <span class="spinner" aria-hidden="true"></span>{t('ob.hw.measuring')}
  </p>
{:else}
  <dl class="tiles">
    {#each tiles as tile (tile.key)}
      <div class="tile" class:wide={tile.key === 'cpu' || tile.key === 'gpu'}>
        <dt>{tile.label}</dt>
        <dd>{tile.value}</dd>
      </div>
    {/each}
  </dl>
  {#if profile.recommendation.tradeoffs.length > 0}
    <section class="notes" aria-labelledby="ob-hw-notes">
      <h3 id="ob-hw-notes">{t('ob.hw.notes')}</h3>
      <ul>
        {#each profile.recommendation.tradeoffs as item (item.pl)}<li>
            {app.i18n.text(item)}
          </li>{/each}
      </ul>
    </section>
  {/if}
{/if}

<style>
  .busy {
    display: flex;
    align-items: center;
    gap: var(--alfa-space-2);
    margin: 0;
    color: var(--alfa-color-text-muted);
  }
  .spinner {
    width: 16px;
    height: 16px;
    border: 2px solid var(--alfa-color-border-strong);
    border-top-color: var(--alfa-color-text);
    border-radius: var(--alfa-radius-full);
    animation: spin 900ms linear infinite;
  }
  :global([data-motion='off']) .spinner {
    animation: none;
  }
  @media (prefers-reduced-motion: reduce) {
    .spinner {
      animation: none;
    }
  }
  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }
  .tiles {
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    gap: var(--alfa-space-2);
    margin: 0;
  }
  .tile {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
    padding: var(--alfa-space-3);
    border: 1px solid var(--alfa-color-border);
    border-radius: var(--alfa-radius-card);
    background: var(--alfa-color-surface2);
  }
  .tile.wide {
    grid-column: span 3;
  }
  dt {
    color: var(--alfa-color-text-muted);
    font-size: var(--alfa-font-size-xs);
  }
  dd {
    margin: 0;
    font-size: var(--alfa-font-size-md);
    font-weight: var(--alfa-weight-semibold);
    overflow-wrap: anywhere;
  }
  .notes {
    display: flex;
    flex-direction: column;
    gap: var(--alfa-space-1);
  }
  h3 {
    margin: 0;
    font-size: var(--alfa-font-size-sm);
  }
  ul {
    margin: 0;
    padding-left: var(--alfa-space-4);
    color: var(--alfa-color-text-muted);
    font-size: var(--alfa-font-size-sm);
    line-height: 1.5;
  }
  @media (max-width: 760px) {
    .tiles {
      grid-template-columns: 1fr;
    }
    .tile.wide {
      grid-column: auto;
    }
  }
</style>
