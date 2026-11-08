<!--
  „Gdzie trafiają dane” (Ustawienia → Dobór modelu, C4.1): każda trasa do dostawcy (API, most CLI)
  z jurysdykcją, tagami prywatności, ryzykiem trenowania, statusem w rejestrze zgodności i tym, czy
  obsłuży sesję prywatną (z powodem). Tylko do odczytu — rejestr i polityka należą do Jądra.
-->
<script lang="ts">
  import { Switch } from '@alfa/ui-kit';
  import { errorText } from '../../../api/command-error';
  import type { DataRoute, DataRoutesView } from '../../../api/types-routes';
  import LoadFailed from '../../../components/shell/LoadFailed.svelte';
  import Loading from '../../../components/shell/Loading.svelte';
  import { useApp } from '../../../state/context';
  import './work.css';

  const app = useApp();
  const { t, tk } = app.i18n;
  let view = $state<DataRoutesView | null>(null);
  let loadError = $state<string | null>(null);
  let onlyEnabled = $state(true);
  const shown = $derived((view?.routes ?? []).filter((r) => !onlyEnabled || r.enabled));

  async function load() {
    try {
      view = await app.client.dataRoutes.list();
      loadError = null;
    } catch (e) {
      loadError = errorText(e);
    }
  }

  $effect(() => {
    void load();
  });

  function where(r: DataRoute): string {
    if (!r.jurisdiction.length) return t('routes.place.unknown');
    return r.jurisdiction
      .map((code) => {
        const key = `routes.place.${code}`;
        const name = tk(key);
        return name === key ? code : name;
      })
      .join(', ');
  }

  function meta(r: DataRoute): string {
    const parts = [tk(`routes.status.${r.status}`)];
    if (r.stale) parts.push(t('routes.stale'));
    if (r.unverified) parts.push(t('routes.unverified'));
    if (r.privacy.length) parts.push(`${t('routes.tags')}: ${r.privacy.join(', ')}`);
    return parts.join(' · ');
  }
</script>

<section class="wk-card" aria-labelledby="routes-title">
  <h3 id="routes-title">{t('routes.title')}</h3>
  <p>{t('routes.intro')}</p>
  {#if loadError}<LoadFailed error={loadError} onretry={() => void load()} />
  {:else if !view}<Loading />
  {:else}
    <Switch
      label={t('routes.onlyEnabled')}
      checked={onlyEnabled}
      onchange={(on) => (onlyEnabled = on)}
    />
    <ul class="wk-list" aria-label={t('routes.list')}>
      {#each shown as r (r.id)}
        <li>
          <div class="wk-actions">
            <strong>{r.provider}</strong>
            <span class="wk-meta">{tk(`routes.mode.${r.mode}`)}</span>
            <span class="wk-meta">{t('routes.where', { place: where(r) })}</span>
            {#if !r.enabled}<span class="wk-meta">{t('routes.disabled')}</span>{/if}
          </div>
          <span class:wk-ok={r.training === 'no_train'} class:wk-warn={r.training !== 'no_train'}>
            {tk(`routes.training.${r.training}`)}
          </span>
          <span class="wk-meta">{meta(r)}</span>
          <span class:wk-ok={r.private_allowed} class:wk-warn={!r.private_allowed}>
            {r.private_allowed
              ? t('routes.private.yes')
              : t('routes.private.no', { why: r.private_reason })}
          </span>
        </li>
      {/each}
    </ul>
  {/if}
</section>
