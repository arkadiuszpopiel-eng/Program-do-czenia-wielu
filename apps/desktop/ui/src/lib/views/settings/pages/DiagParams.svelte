<!--
  Parametry komputera z ostatniego zapisu (diagnostyka): system, procesor, pamięć z modułami,
  dyski z kondycją i błędami z dziennika, woluminy, karty graficzne, sieć, bateria, temperatury,
  zdarzenia awarii i części, których nie udało się odczytać. Treść zdarzeń pochodzi z dziennika
  systemu — pokazywana jako zwykły tekst.
-->
<script lang="ts">
  import type { Snapshot } from '../../../api/types-diag';
  import { gb, uptime } from '../../../logic/diag';
  import { useApp } from '../../../state/context';
  import './work.css';

  let { snap }: { snap: Snapshot } = $props();
  const app = useApp();
  const { t, tk } = app.i18n;
  const pct = (a: number, b: number) => (b ? Math.round((100 * a) / b) : 0);
  const dash = (v: string | number | null | undefined, unit = '') =>
    v === null || v === undefined || v === '' ? '—' : `${v}${unit}`;
  const wear = $derived.by(() => {
    const b = snap.battery;
    if (!b?.design_mwh || !b.full_mwh) return null;
    return Math.max(0, Math.round(100 * (1 - b.full_mwh / b.design_mwh)));
  });
</script>

<section class="wk-card" aria-labelledby="diag-params">
  <h3 id="diag-params">{t('diag.params')}</h3>
  <div class="grid">
    <div>
      <h4>{t('diag.sec.system')}</h4>
      <p>{snap.machine.os} ({snap.machine.os_version})</p>
      <p class="wk-meta">{dash(snap.machine.model)} · BIOS {dash(snap.machine.bios)}</p>
      <p class="wk-meta">{t('diag.uptime')}: {uptime(snap.machine.uptime_s)}</p>
    </div>
    <div>
      <h4>{t('diag.sec.cpu')}</h4>
      <p>{snap.cpu.name}</p>
      <p class="wk-meta">
        {t('diag.cores', { cores: snap.cpu.cores, threads: snap.cpu.threads })} · {t('diag.clock')}
        {dash(snap.cpu.current_mhz)} / {dash(snap.cpu.base_mhz, ' MHz')} · {t('diag.load')}
        {dash(snap.cpu.load_pct, '%')}
      </p>
      <p class="wk-meta">
        {t('diag.cache')}: {dash(snap.cpu.l2_kb)} / {dash(snap.cpu.l3_kb, ' KB')}
      </p>
    </div>
    <div>
      <h4>{t('diag.sec.memory')}</h4>
      <p>
        {t('diag.memUsed', {
          used: gb(snap.memory.total_mb - snap.memory.available_mb),
          total: gb(snap.memory.total_mb),
          pct: pct(snap.memory.total_mb - snap.memory.available_mb, snap.memory.total_mb),
        })}
      </p>
      {#each snap.memory.modules as m, i (i)}
        <p class="wk-meta">
          {dash(m.slot)}: {gb(m.capacity_mb)} · {dash(m.speed_mhz, ' MHz')} · {dash(m.manufacturer)}
          {dash(m.part)}
        </p>
      {/each}
      {#if snap.memory.pagefile_mb !== null}
        <p class="wk-meta">
          {t('diag.pagefile')}: {gb(snap.memory.pagefile_mb)} ({gb(
            snap.memory.pagefile_used_mb ?? 0,
          )})
        </p>
      {/if}
    </div>
  </div>

  <h4>{t('diag.sec.disks')}</h4>
  <ul class="wk-list">
    {#each snap.disks as d, i (i)}
      <li>
        <strong>{d.model}</strong>
        <span class="wk-meta">
          {d.media.toUpperCase()} · {dash(d.bus)} · {gb(d.size_mb)} · {d.volumes.join(', ') || '—'}
        </span>
        <span
          class={d.health === 'healthy' ? 'wk-ok' : d.health === 'unknown' ? 'wk-meta' : 'wk-error'}
          >{tk(`diag.health.${d.health}`)}</span
        >
        {#if d.event_errors > 0}
          <span class="wk-warn">{t('diag.diskErrors', { n: d.event_errors })}</span>
        {/if}
        {#if d.wear_pct !== null || d.temperature_c !== null || d.power_on_hours !== null}
          <span class="wk-meta">
            {dash(d.wear_pct, '%')} · {dash(d.temperature_c, ' °C')} · {dash(
              d.power_on_hours,
              ' h',
            )}
          </span>
        {/if}
      </li>
    {/each}
    {#each snap.volumes as v (v.letter)}
      <li>
        <span>
          <strong>{v.letter}</strong>
          {dash(v.label)} ({dash(v.file_system)}) — {t('diag.free', {
            free: gb(v.free_mb),
            size: gb(v.size_mb),
            pct: pct(v.free_mb, v.size_mb),
          })}
        </span>
      </li>
    {/each}
  </ul>

  <div class="grid">
    <div>
      <h4>{t('diag.sec.gpus')}</h4>
      {#each snap.gpus as g, i (i)}
        <p>{g.name} · {g.vram_mb === null ? '—' : gb(g.vram_mb)}</p>
        <p class="wk-meta">
          {t('diag.driver', { version: dash(g.driver_version), date: dash(g.driver_date) })} ·
          {dash(g.status)}
        </p>
      {/each}
    </div>
    <div>
      <h4>{t('diag.sec.network')}</h4>
      {#each snap.network as n, i (i)}
        <p>
          {n.name} ({n.kind}) — {n.up ? t('diag.up') : t('diag.down')}{n.link_mbps
            ? `, ${n.link_mbps} Mb/s`
            : ''}
        </p>
      {/each}
    </div>
    <div>
      {#if snap.battery}
        <h4>{t('diag.sec.battery')}</h4>
        <p>
          {dash(snap.battery.charge_pct, '%')} · {snap.battery.on_ac
            ? t('diag.onAc')
            : t('diag.onBattery')}{wear !== null
            ? ` · ${t('diag.batteryWear', { pct: wear })}`
            : ''}
        </p>
      {/if}
      {#if snap.thermal.length}
        <h4>{t('diag.sec.thermal')}</h4>
        {#each snap.thermal as z, i (i)}
          <p class={z.celsius >= 90 ? 'wk-error' : z.celsius >= 80 ? 'wk-warn' : ''}>
            {z.name}: {Math.round(z.celsius)} °C
          </p>
        {/each}
      {/if}
    </div>
  </div>

  <h4>{t('diag.sec.events', { days: snap.events.window_days })}</h4>
  {#if snap.events.counts.length === 0}
    <p class="wk-meta">{t('diag.noEvents')}</p>
  {:else}
    <p>
      {snap.events.counts.map((c) => `${tk(`diag.event.${c.kind}`)}: ${c.count}`).join(' · ')}
    </p>
    <ul class="wk-plain events">
      {#each snap.events.recent.slice(0, 12) as e, i (i)}
        <li>
          <span class="wk-meta">{app.i18n.dateTime(e.at)} · {e.source} ({e.id})</span>
          <span class="msg">{e.message}</span>
        </li>
      {/each}
    </ul>
  {/if}
  {#if snap.gaps.length}
    <h4>{t('diag.sec.gaps')}</h4>
    <ul class="wk-plain">
      {#each snap.gaps as g, i (i)}<li class="wk-meta">{g}</li>{/each}
    </ul>
  {/if}
</section>

<style>
  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(16rem, 1fr));
    gap: var(--alfa-space-3);
  }
  .grid p {
    margin: 0;
  }
  .events li {
    display: flex;
    flex-direction: column;
    margin-bottom: var(--alfa-space-1);
  }
  .msg {
    overflow-wrap: anywhere;
  }
</style>
