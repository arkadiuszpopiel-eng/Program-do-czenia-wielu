<!--
  Zdrowie systemu → Ulepszacz i evale: propozycje (pierścień, klasa bezpieczeństwa, etap, diff
  kluczy z digestem — zatwierdzasz dokładnie ten diff; R1/R2 wymagają podpisu), wycofanie,
  zablokowane próby, „Przeanalizuj teraz" (cykl w bezczynności tylko z portem bezczynności),
  zamrożone zestawy evali (integralność hashem) i werdykty bramki (holdout — tylko wynik zbiorczy).
-->
<script lang="ts">
  import { Button } from '@alfa/ui-kit';
  import type { EvalsView, ImproverView } from '../../../api/types-work';
  import LoadFailed from '../../../components/shell/LoadFailed.svelte';
  import { attempt, load } from '../../../state/attempt';
  import { useApp } from '../../../state/context';

  const app = useApp();
  const { t, tk } = app.i18n;
  let improver = $state<ImproverView | null>(null);
  let evals = $state<EvalsView | null>(null);
  let loadError = $state<string | null>(null);

  async function reload() {
    const result = await load(() =>
      Promise.all([app.client.health.improver(), app.client.health.evals()]),
    );
    if (result.status === 'ready') {
      [improver, evals] = result.value;
      loadError = null;
    } else if (result.status === 'failed') loadError = result.error;
  }

  $effect(() => {
    void reload();
  });

  $effect(() =>
    app.on((event) => {
      if (event.type === 'HealthChanged') void reload();
    }),
  );

  async function run(action: () => Promise<unknown>, message?: string) {
    if (!(await attempt(app.toasts, action))) return;
    if (message) app.toasts.show({ kind: 'success', message });
    await reload();
  }

  const show = (v: unknown) => JSON.stringify(v);
</script>

<section class="wk-card" aria-labelledby="im-title">
  <h3 id="im-title">{t('improver.title')}</h3>
  <p>{t('improver.intro')}</p>
  {#if loadError}<LoadFailed error={loadError} onretry={() => void reload()} />{/if}
  {#if improver}
    <p class="wk-meta">
      {improver.idle_cycle ? t('improver.idle') : t('improver.manual')}
      {#if improver.last_cycle}· {t('improver.last', {
          time: app.i18n.dateTime(improver.last_cycle),
        })}{/if}
    </p>
  {/if}
  <div class="wk-actions">
    <Button
      size="sm"
      variant="secondary"
      onclick={() => run(() => app.client.health.improverCycle(), t('improver.cycled'))}
      >{t('improver.cycle')}</Button
    >
  </div>
  {#if improver}
    {#if improver.proposals.length === 0}
      <p class="wk-meta">{t('improver.empty')}</p>
    {:else}
      <ul class="wk-list">
        {#each improver.proposals as p (p.id)}
          <li>
            <strong>{p.title}</strong>
            <span>{p.rationale}</span>
            <span class="wk-meta"
              >{p.ring} · {tk(`improver.safety.${p.safety}`)} · {tk(`improver.stage.${p.stage}`)} · {p.source}</span
            >
            {#if p.note}<span class="wk-note">{p.note}</span>{/if}
            {#if p.changes.length}
              <table class="wk-table">
                <thead>
                  <tr>
                    <th scope="col">{t('improver.key')}</th>
                    <th scope="col">{t('improver.old')}</th>
                    <th scope="col">{t('improver.new')}</th>
                  </tr>
                </thead>
                <tbody>
                  {#each p.changes as c (c.key)}
                    <tr
                      ><td><code>{c.key}</code></td><td>{show(c.old)}</td><td>{show(c.new)}</td></tr
                    >
                  {/each}
                </tbody>
              </table>
            {/if}
            <span class="wk-meta">digest: <code class="wk-code">{p.digest}</code></span>
            {#if p.needs_signature}<span class="wk-note">{t('improver.signature')}</span>{/if}
            <div class="wk-actions">
              {#if p.can_approve}
                <Button
                  size="sm"
                  variant="primary"
                  aria-label={`${t('improver.approve')}: ${p.title}`}
                  onclick={() =>
                    run(
                      () => app.client.health.improverApprove(p.id, p.digest),
                      t('improver.approved'),
                    )}>{t('improver.approve')}</Button
                >
                <Button
                  size="sm"
                  variant="secondary"
                  aria-label={`${t('improver.reject')}: ${p.title}`}
                  onclick={() => run(() => app.client.health.improverReject(p.id))}
                  >{t('improver.reject')}</Button
                >
              {/if}
              {#if p.can_rollback}
                <Button
                  size="sm"
                  variant="ghost"
                  aria-label={`${t('improver.rollback')}: ${p.title}`}
                  onclick={() =>
                    run(() => app.client.health.improverRollback(p.id), t('improver.rolledBack'))}
                  >{t('improver.rollback')}</Button
                >
              {/if}
            </div>
          </li>
        {/each}
      </ul>
    {/if}
    {#if improver.blocked.length}
      <h4>{t('improver.blocked')}</h4>
      <ul class="wk-plain wk-meta">
        {#each improver.blocked as b, i (i)}
          <li>{app.i18n.dateTime(b.at)} · {b.source} → {b.target}: {b.violation}</li>
        {/each}
      </ul>
    {/if}
  {/if}
</section>

<section class="wk-card" aria-labelledby="ev-title">
  <h3 id="ev-title">{t('evals.title')}</h3>
  {#if evals}
    {#if !evals.available}<p class="wk-note">{evals.reason ?? t('evals.unavailable')}</p>{/if}
    <p class="wk-meta">{t('evals.holdout', { n: evals.holdout_suites })}</p>
    {#if evals.suites.length}
      <table class="wk-table">
        <thead>
          <tr>
            <th scope="col">{t('evals.suite')}</th>
            <th scope="col">{t('evals.integrity')}</th>
            <th scope="col">{t('evals.thresholds')}</th>
            <th scope="col"><span class="alfa-visually-hidden">{t('evals.verify')}</span></th>
          </tr>
        </thead>
        <tbody>
          {#each evals.suites as s (s.id)}
            <tr>
              <td><code>{s.id}</code> {s.version} · {s.wave}</td>
              <td class:wk-error={!s.integrity_ok}
                >{s.integrity_ok ? t('evals.ok') : s.problems.join('; ') || t('evals.broken')}</td
              >
              <td>{s.thresholds}</td>
              <td>
                <Button
                  size="sm"
                  variant="ghost"
                  aria-label={`${t('evals.verify')}: ${s.id}`}
                  onclick={() =>
                    run(() => app.client.health.evalsVerify(s.id), t('evals.verified'))}
                  >{t('evals.verify')}</Button
                >
              </td>
            </tr>
          {/each}
        </tbody>
      </table>
    {:else}
      <p class="wk-meta">{t('evals.empty')}</p>
    {/if}
    {#if evals.verdicts.length}
      <h4>{t('evals.verdicts')}</h4>
      <ul class="wk-plain wk-meta">
        {#each evals.verdicts as v, i (i)}
          <li>
            {app.i18n.dateTime(v.at)} · {v.suite} · {v.stage}: {v.passed
              ? t('evals.passed')
              : t('evals.failed')} — {v.summary}
          </li>
        {/each}
      </ul>
    {/if}
  {/if}
</section>
