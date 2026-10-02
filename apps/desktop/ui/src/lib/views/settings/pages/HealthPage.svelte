<!--
  Ustawienia → Zdrowie systemu (Diagnosta): stan ogólny, moduły (cykl życia, zdrowie), incydenty,
  propozycje napraw (zgoda; naprawy Jądra — wyłącznie okno Brokera), wykonane naprawy z „Cofnij",
  sprawy dla człowieka. Niżej Ulepszacz i evale (ImproverSection).
-->
<script lang="ts">
  import { Button } from '@alfa/ui-kit';
  import type { HealthView } from '../../../api/types-work';
  import { useApp } from '../../../state/context';
  import ImproverSection from './ImproverSection.svelte';
  import './work.css';

  const app = useApp();
  const { t, tk } = app.i18n;
  let view = $state<HealthView | null>(null);

  async function load() {
    view = await app.client.health.report();
  }

  $effect(() => {
    void load();
  });

  $effect(() =>
    app.on((event) => {
      if (event.type === 'HealthChanged') void load();
    }),
  );

  async function run(action: () => Promise<HealthView>, message?: string) {
    try {
      view = await action();
      if (message) app.toasts.show({ kind: 'success', message });
    } catch (error) {
      app.toasts.show({
        kind: 'error',
        message: error instanceof Error ? error.message : String(error),
      });
    }
  }
</script>

<section class="wk-card" aria-labelledby="hl-title">
  <h3 id="hl-title">{t('health.title')}</h3>
  <p>{t('health.intro')}</p>
  {#if view}
    <p role="status" class:wk-ok={view.overall === 'ok'} class:wk-warn={view.overall !== 'ok'}>
      {tk(`health.overall.${view.overall}`)} · {t('health.generated', {
        time: app.i18n.time(view.generated_at),
      })}
    </p>
    {#if view.safe_mode}<p class="wk-warn">{t('health.safeMode', { why: view.safe_mode })}</p>{/if}
    {#each view.problems as p (p)}<p class="wk-note">{p}</p>{/each}
  {/if}
  <div class="wk-actions">
    <Button
      size="sm"
      variant="secondary"
      onclick={() => run(() => app.client.health.scan(), t('health.scanned'))}
      >{t('health.scan')}</Button
    >
  </div>
</section>

{#if view}
  {#if view.pending.length}
    <section class="wk-card wk-accent" aria-labelledby="hl-pending">
      <h3 id="hl-pending">{t('health.pending')}</h3>
      <ul class="wk-list">
        {#each view.pending as p (p.id)}
          <li>
            <strong>{p.title}</strong>
            <span>{p.rationale}</span>
            <span class="wk-meta"
              >{t('health.risk', { risk: tk(`health.riskLevel.${p.risk}`) })}</span
            >
            <pre class="wk-diff">{p.diff.join('\n')}</pre>
            <span class="wk-meta">{t('health.rollback')}: {p.rollback_plan.join('; ') || '—'}</span>
            {#if p.kernel}<span class="wk-note">{t('health.kernel')}</span>{/if}
            <div class="wk-actions">
              <Button
                size="sm"
                variant="primary"
                aria-label={`${t('health.approve')}: ${p.title}`}
                onclick={() => run(() => app.client.health.approve(p.id), t('health.approved'))}
                >{p.kernel ? t('health.approveBroker') : t('health.approve')}</Button
              >
              <Button
                size="sm"
                variant="secondary"
                aria-label={`${t('health.reject')}: ${p.title}`}
                onclick={() => run(() => app.client.health.reject(p.id))}
                >{t('health.reject')}</Button
              >
            </div>
          </li>
        {/each}
      </ul>
    </section>
  {/if}

  <section class="wk-card" aria-labelledby="hl-incidents">
    <h3 id="hl-incidents">{t('health.incidents')}</h3>
    {#if view.incidents.length === 0}
      <p class="wk-meta">{t('health.incidentsEmpty')}</p>
    {:else}
      <ul class="wk-list">
        {#each view.incidents as i (i.id)}
          <li>
            <strong>{i.title}</strong>
            <span class="wk-meta"
              >{i.target} · {t('health.count', { n: i.count })} · {app.i18n.dateTime(i.last_at)} · {i.status}</span
            >
          </li>
        {/each}
      </ul>
    {/if}
  </section>

  <section class="wk-card" aria-labelledby="hl-repaired">
    <h3 id="hl-repaired">{t('health.repaired')}</h3>
    {#if view.repaired.length === 0}
      <p class="wk-meta">{t('health.repairedEmpty')}</p>
    {:else}
      <ul class="wk-list">
        {#each view.repaired as r (r.id)}
          <li>
            <strong>{r.title}</strong>
            <span class="wk-meta">{app.i18n.dateTime(r.at)} · {r.diff.join('; ')}</span>
            {#if r.undoable}
              <div class="wk-actions">
                <Button
                  size="sm"
                  variant="ghost"
                  aria-label={`${t('common.undo')}: ${r.title}`}
                  onclick={() => run(() => app.client.health.undo(r.id), t('health.undone'))}
                  >{t('common.undo')}</Button
                >
              </div>
            {/if}
          </li>
        {/each}
      </ul>
    {/if}
  </section>

  {#if view.needs_human.length}
    <section class="wk-card" aria-labelledby="hl-human">
      <h3 id="hl-human">{t('health.human')}</h3>
      <ul class="wk-list">
        {#each view.needs_human as h (h.id)}
          <li>
            <strong>{h.title}</strong>
            <span>{h.what}</span>
            {#if h.mitigated}<span class="wk-meta">{t('health.mitigated')}</span>{/if}
          </li>
        {/each}
      </ul>
    </section>
  {/if}

  <section class="wk-card" aria-labelledby="hl-modules">
    <h3 id="hl-modules">{t('health.modules')}</h3>
    <table class="wk-table">
      <thead>
        <tr>
          <th scope="col">{t('health.module')}</th>
          <th scope="col">{t('health.state')}</th>
          <th scope="col">{t('health.detail')}</th>
        </tr>
      </thead>
      <tbody>
        {#each view.modules as m (m.module)}
          <tr>
            <td><code>{m.module}</code> {m.version}</td>
            <td class:wk-warn={m.health !== 'healthy'}>{tk(`health.module.${m.health}`)}</td>
            <td>{m.detail ?? '—'}</td>
          </tr>
        {/each}
      </tbody>
    </table>
  </section>
{/if}

<ImproverSection />
