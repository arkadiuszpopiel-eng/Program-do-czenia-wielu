<!--
  Karty graficzne na żywo (strona „Urządzenia”): pamięć zajęta i wolna, obciążenie, temperatura,
  moc oraz programy korzystające z karty — z zaznaczeniem silników Alfy. Odświeża się samo co kilka
  sekund, dopóki strona jest otwarta; pomiar działa dziś dla kart NVIDIA.
-->
<script lang="ts">
  import { Button } from '@alfa/ui-kit';
  import type { GpuLive } from '../../../api/types-hub';
  import { load } from '../../../state/attempt';
  import { useApp } from '../../../state/context';

  const REFRESH_MS = 3_000;

  const app = useApp();
  const { t } = app.i18n;
  let live = $state<GpuLive | null>(null);
  let error = $state<string | null>(null);
  let paused = $state(false);

  async function read() {
    const result = await load(() => app.client.device.gpuLive());
    if (result.status === 'ready') {
      live = result.value;
      error = null;
    } else if (result.status === 'failed') error = result.error;
  }

  $effect(() => {
    void read();
  });

  $effect(() => {
    if (paused) return;
    const handle = setInterval(() => void read(), REFRESH_MS);
    return () => clearInterval(handle);
  });

  const gb = (mb: number) =>
    (mb / 1024).toLocaleString(app.i18n.locale, { maximumFractionDigits: 1 });
  const share = (used: number, total: number) =>
    total > 0 ? Math.min(100, Math.round((used / total) * 100)) : 0;
  const alfaMb = $derived(
    live?.processes.reduce((sum, p) => sum + (p.alfa ? (p.used_mb ?? 0) : 0), 0) ?? 0,
  );
</script>

<section class="card" aria-labelledby="gpu-live-title" data-testid="gpu-live">
  <div class="head">
    <h3 id="gpu-live-title">{t('dev.live.title')}</h3>
    <div class="actions">
      <Button size="sm" variant="ghost" onclick={() => (paused = !paused)}
        >{t(paused ? 'dev.live.resume' : 'dev.live.pause')}</Button
      >
      <Button size="sm" variant="secondary" onclick={() => void read()}
        >{t('dev.live.refresh')}</Button
      >
    </div>
  </div>
  {#if error}
    <p class="warn" role="status">{error}</p>
  {:else if !live}
    <p class="muted">{t('dev.live.loading')}</p>
  {:else if !live.available}
    <p class="muted">{t('dev.live.unavailable')}</p>
  {:else}
    {#each live.cards as card (card.index)}
      {@const pct = share(card.used_mb, card.total_mb)}
      <div class="gpu">
        <div class="gpu-top">
          <span class="name">{card.name}</span>
          <span class="mem"
            >{t('dev.live.memory', {
              used: gb(card.used_mb),
              total: gb(card.total_mb),
              free: gb(card.free_mb),
            })}</span
          >
        </div>
        <div
          class="bar"
          role="meter"
          aria-label={t('dev.live.memoryLabel', { name: card.name })}
          aria-valuemin="0"
          aria-valuemax="100"
          aria-valuenow={pct}
          aria-valuetext={t('dev.live.memoryPct', { pct })}
        >
          <span class="fill" class:high={pct >= 90} style:width="{pct}%"></span>
        </div>
        <dl class="stats">
          <div>
            <dt>{t('dev.live.load')}</dt>
            <dd>{card.load_pct === null ? '—' : `${card.load_pct} %`}</dd>
          </div>
          <div>
            <dt>{t('dev.live.temp')}</dt>
            <dd>{card.temp_c === null ? '—' : `${card.temp_c} °C`}</dd>
          </div>
          <div>
            <dt>{t('dev.live.power')}</dt>
            <dd>{card.power_w === null ? '—' : `${card.power_w} W`}</dd>
          </div>
          <div>
            <dt>{t('dev.live.alfaShare')}</dt>
            <dd>{alfaMb > 0 ? `${gb(alfaMb)} GB` : '—'}</dd>
          </div>
        </dl>
      </div>
    {/each}
    <h4>{t('dev.live.processes')}</h4>
    {#if live.processes.length === 0}
      <p class="muted">{t('dev.live.noProcesses')}</p>
    {:else}
      <ul class="procs">
        {#each live.processes as p (p.pid)}
          <li class:alfa={p.alfa}>
            <span class="pname">{p.name}</span>
            {#if p.alfa}<span class="tag">{t('dev.live.alfa')}</span>{/if}
            <span class="pmem">{p.used_mb === null ? '—' : `${p.used_mb} MB`}</span>
          </li>
        {/each}
      </ul>
      <p class="muted">{t('dev.live.processNote')}</p>
    {/if}
  {/if}
</section>

<style>
  .card {
    display: flex;
    flex-direction: column;
    gap: var(--alfa-space-3);
    margin-bottom: var(--alfa-space-4);
    padding: var(--alfa-space-4);
    border: 1px solid var(--alfa-color-border);
    border-radius: var(--alfa-radius-card);
    background: var(--alfa-color-surface);
  }
  .head,
  .gpu-top {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: space-between;
    gap: var(--alfa-space-2);
  }
  .actions {
    display: flex;
    gap: var(--alfa-space-2);
  }
  h3 {
    font-size: var(--alfa-font-size-md);
  }
  h4 {
    margin: 0;
    font-size: var(--alfa-font-size-sm);
  }
  .gpu {
    display: flex;
    flex-direction: column;
    gap: var(--alfa-space-2);
  }
  .name {
    font-weight: var(--alfa-weight-semibold);
  }
  .mem,
  .muted {
    color: var(--alfa-color-text-muted);
    font-size: var(--alfa-font-size-sm);
  }
  .muted {
    margin: 0;
    font-size: var(--alfa-font-size-xs);
  }
  .warn {
    margin: 0;
    padding-left: var(--alfa-space-2);
    border-left: 3px solid var(--alfa-color-warning);
    font-size: var(--alfa-font-size-sm);
  }
  .bar {
    height: 10px;
    overflow: hidden;
    border: 1px solid var(--alfa-color-border);
    border-radius: var(--alfa-radius-full);
    background: var(--alfa-color-surface2);
  }
  .fill {
    display: block;
    height: 100%;
    border-radius: var(--alfa-radius-full);
    background: var(--alfa-color-accent);
  }
  .fill.high {
    background: var(--alfa-color-warning);
  }
  .stats {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(8rem, 1fr));
    gap: var(--alfa-space-2);
    margin: 0;
  }
  .stats > div {
    padding: var(--alfa-space-2);
    border: 1px solid var(--alfa-color-border);
    border-radius: var(--alfa-radius-control);
    background: var(--alfa-color-surface2);
  }
  .stats dt {
    color: var(--alfa-color-text-muted);
    font-size: var(--alfa-font-size-xs);
  }
  .stats dd {
    margin: 0;
    font-weight: var(--alfa-weight-semibold);
    font-variant-numeric: tabular-nums;
  }
  .procs {
    display: flex;
    flex-direction: column;
    gap: 2px;
    margin: 0;
    padding: 0;
    list-style: none;
    font-size: var(--alfa-font-size-sm);
  }
  .procs li {
    display: flex;
    align-items: center;
    gap: var(--alfa-space-2);
    padding: var(--alfa-space-1) var(--alfa-space-2);
    border-radius: var(--alfa-radius-control);
  }
  .procs li.alfa {
    background: var(--alfa-color-surface2);
  }
  .pname {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .tag {
    padding: 0 var(--alfa-space-2);
    border: 1px solid var(--alfa-color-border-strong);
    border-radius: var(--alfa-radius-full);
    font-size: var(--alfa-font-size-xs);
    white-space: nowrap;
  }
  .pmem {
    margin-left: auto;
    color: var(--alfa-color-text-muted);
    font-variant-numeric: tabular-nums;
    white-space: nowrap;
  }
</style>
