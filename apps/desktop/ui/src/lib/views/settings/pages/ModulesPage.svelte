<!--
  Ustawienia → Moduły: każdy moduł Alfy ze stanem, rodzajem, cyklem życia, izolacją, budżetem RAM /
  CPU / karty, zdolnościami i zależnościami. Moduły Jądra są chronione; pięć modułów opcjonalnych
  można wyłączyć — zmiana działa od następnego uruchomienia (baner).
-->
<script lang="ts">
  import { Switch, TextField } from '@alfa/ui-kit';
  import { errorText } from '../../../api/command-error';
  import type { ModuleRow, ModulesView } from '../../../api/types-modules';
  import LoadFailed from '../../../components/shell/LoadFailed.svelte';
  import Loading from '../../../components/shell/Loading.svelte';
  import { useApp } from '../../../state/context';
  import './work.css';

  const app = useApp();
  const { t, tk } = app.i18n;
  let view = $state<ModulesView | null>(null);
  let loadError = $state<string | null>(null);
  let query = $state('');
  let busy = $state<string | null>(null);
  const shown = $derived(
    (view?.modules ?? []).filter((m) => m.id.includes(query.trim().toLowerCase())),
  );
  const optional = $derived(shown.filter((m) => m.can_disable));
  const others = $derived(shown.filter((m) => !m.can_disable));

  async function load() {
    try {
      view = await app.client.modules.list();
      loadError = null;
    } catch (e) {
      loadError = errorText(e);
    }
  }

  $effect(() => {
    void load();
  });

  async function toggle(m: ModuleRow, on: boolean) {
    busy = m.id;
    try {
      view = await app.client.modules.setEnabled(m.id, on);
    } catch (e) {
      app.toasts.show({ kind: 'error', message: errorText(e) });
    } finally {
      busy = null;
    }
  }

  function stateClass(state: string): string {
    if (state === 'ready') return 'wk-ok';
    if (state === 'failed') return 'wk-error';
    if (state === 'degraded') return 'wk-warn';
    return 'wk-meta';
  }

  function budget(m: ModuleRow): string {
    const vram = m.vram_mb ? ` · ${t('modules.vram', { mb: m.vram_mb })}` : '';
    return `${t('modules.ram', { mb: m.ram_mb })} · CPU ${m.cpu_pct}%${vram}`;
  }
</script>

{#snippet item(m: ModuleRow)}
  <li>
    <div class="wk-actions">
      <strong class="wk-code">{m.id}</strong>
      <span class={stateClass(m.state)}>{tk(`modules.state.${m.state}`)}</span>
      {#if m.protected}<span class="wk-meta">{t('modules.protected')}</span>{/if}
      {#if m.can_disable}
        <Switch
          label={t('modules.enabledFor', { id: m.id })}
          checked={!m.disabled}
          disabled={busy !== null}
          onchange={(on) => void toggle(m, on)}
        />
      {/if}
    </div>
    <span class="wk-meta">
      {m.version} · {tk(`modules.kind.${m.kind}`)} · {tk(`modules.lifecycle.${m.lifecycle}`)} ·
      {m.isolation} · {budget(m)}
    </span>
    {#if m.detail}<span class="wk-warn">{m.detail}</span>{/if}
    {#if m.capabilities.length}
      <span class="wk-meta">{t('modules.capabilities')}: {m.capabilities.join(', ')}</span>
    {/if}
  </li>
{/snippet}

<section class="wk-card" aria-labelledby="modules-title">
  <h3 id="modules-title">{t('modules.title')}</h3>
  <p>{t('modules.intro')}</p>
  {#if view}
    <p class="wk-meta">
      {t('modules.summary', { n: view.modules.length, ram: Math.round(view.ram_total_mb / 1024) })}
    </p>
    {#if view.restart_needed}
      <p class="wk-warn" role="status">{t('modules.restart')}</p>
    {/if}
    <TextField label={t('modules.search')} bind:value={query} spellcheck="false" />
  {/if}
  {#if loadError}<LoadFailed error={loadError} onretry={() => void load()} />
  {:else if !view}<Loading />{/if}
</section>

{#if view}
  <section class="wk-card" aria-labelledby="modules-optional">
    <h3 id="modules-optional">{t('modules.optional')}</h3>
    <p class="wk-meta">{t('modules.optionalHint')}</p>
    <ul class="wk-list">
      {#each optional as m (m.id)}{@render item(m)}{/each}
    </ul>
  </section>
  <section class="wk-card" aria-labelledby="modules-all">
    <h3 id="modules-all">{t('modules.all')}</h3>
    <ul class="wk-list">
      {#each others as m (m.id)}{@render item(m)}{/each}
    </ul>
  </section>
{/if}
