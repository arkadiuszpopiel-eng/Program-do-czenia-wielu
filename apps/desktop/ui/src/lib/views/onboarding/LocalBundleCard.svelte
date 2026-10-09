<!--
  Model lokalny w onboardingu jako pakiet dobrany do tego komputera (model rozmowy razem z silnikiem,
  który go uruchamia). Test na laptopie (2026-10-07): karta pobierała sam model, bez `llama-server`,
  więc pierwsza rozmowa kończyła się błędem. Pakiet zalecany pobiera komplet jednym kliknięciem;
  pliki bez przypiętej sumy zatrzymują się na jawnej zgodzie (jak w Ustawieniach → Modele lokalne i silniki).
  Gdy rdzeń nie zwraca pakietów, zostaje poprzednia karta samego modelu.
-->
<script lang="ts">
  import { Button } from '@alfa/ui-kit';
  import { errorText } from '../../api/command-error';
  import type { ModelBundle, ModelItem } from '../../api/types-models';
  import LoadFailed from '../../components/shell/LoadFailed.svelte';
  import { bundleAction, bundlePercent, recommendedBundle } from '../../logic/bundles';
  import { groupedHash, trustHashes } from '../../logic/models';
  import { load } from '../../state/attempt';
  import { useApp } from '../../state/context';
  import LocalModelCard from './LocalModelCard.svelte';

  const app = useApp();
  const { t } = app.i18n;
  let bundles = $state<readonly ModelBundle[] | null>(null);
  let items = $state<readonly ModelItem[]>([]);
  let loadError = $state<string | null>(null);
  let failure = $state<string | null>(null);
  let busy = $state(false);

  async function reload() {
    const result = await load(() =>
      Promise.all([app.client.engines.bundles(), app.client.engines.list()]),
    );
    if (result.status === 'ready') {
      bundles = result.value[0];
      items = result.value[1].items;
      loadError = null;
    } else if (result.status === 'failed') loadError = result.error;
  }

  // Postęp pobierania przychodzi zdarzeniami pozycji; stan pakietu liczy rdzeń — odświeżamy go
  // najwyżej raz na 400 ms, żeby strumień postępu nie zalewał rdzenia zapytaniami.
  $effect(() => {
    void reload();
    let timer: ReturnType<typeof setTimeout> | null = null;
    const off = app.on((event) => {
      if (event.type !== 'ModelChanged' && event.type !== 'ModelProgress') return;
      timer ??= setTimeout(() => {
        timer = null;
        void reload();
      }, 400);
    });
    return () => {
      off();
      if (timer) clearTimeout(timer);
    };
  });

  // Zalecany dla tej maszyny; gdy rdzeń żadnego nie oznaczył — najwyższy, który nie jest „za słaby”.
  const bundle = $derived.by(() => {
    if (!bundles) return null;
    return (
      recommendedBundle(bundles) ??
      [...bundles].sort((a, b) => b.rating - a.rating).find((b) => b.fit.kind !== 'too_weak') ??
      null
    );
  });
  const action = $derived(bundle ? bundleAction(bundle) : null);
  const percent = $derived(bundle ? bundlePercent(bundle) : null);
  const waiting = $derived.by(() => {
    if (!bundle) return [];
    const ids = new Set(bundle.items.map((i) => i.id));
    return items.filter((i) => ids.has(i.id) && i.state === 'needs_trust');
  });

  async function run(work: () => Promise<unknown>) {
    if (busy) return;
    busy = true;
    failure = null;
    try {
      await work();
      await reload();
    } catch (error) {
      failure = errorText(error);
    } finally {
      busy = false;
    }
  }
</script>

{#if loadError && !bundles}
  <section class="local" aria-labelledby="ob-bundle-title">
    <h3 id="ob-bundle-title">{t('ob.bundle.title')}</h3>
    <LoadFailed error={loadError} onretry={() => void reload()} />
  </section>
{:else if bundles && !bundle}
  <LocalModelCard />
{:else if bundle}
  <section class="local" aria-labelledby="ob-bundle-title">
    <h3 id="ob-bundle-title">{t('ob.bundle.title')}</h3>
    <p class="muted">{t('ob.bundle.desc')}</p>
    <p class="name">
      <strong
        >{t('ob.bundle.name', { rating: bundle.rating, name: app.i18n.text(bundle.name) })}</strong
      >
      <span class="muted small"
        >{t('ob.bundle.contents', {
          n: bundle.total,
          size: app.i18n.bytes(bundle.size_bytes),
        })}</span
      >
    </p>
    <p class="small">{app.i18n.text(bundle.summary)}</p>
    {#if bundle.fit.kind !== 'fits' && bundle.fit.reason}
      <p class="warn small">{app.i18n.text(bundle.fit.reason)}</p>
    {/if}

    {#if bundle.state === 'installed'}
      <p class="ok" role="status">{t('ob.bundle.done')}</p>
    {:else if bundle.state === 'downloading'}
      <progress aria-label={t('ob.bundle.progress')} max="100" value={percent ?? undefined}
      ></progress>
      <p class="muted small" role="status">
        {t('ob.bundle.installed', { done: bundle.installed, total: bundle.total })}
      </p>
    {:else if bundle.state === 'needs_trust'}
      <p class="small" role="status">{t('ob.bundle.trust')}</p>
    {/if}

    {#each waiting as item (item.id)}
      <div class="trust">
        <p><strong>{item.name}</strong></p>
        <p class="small">{t('engines.trust.body')}</p>
        <p class="muted small">{t('engines.trust.license', { license: item.license })}</p>
        {#each item.files.filter((f) => !f.pinned_sha256 && f.sha256) as f (f.name)}
          <p class="small">{f.name}</p>
          <code class="hash">{groupedHash(f.sha256 ?? '')}</code>
        {/each}
        <Button
          variant="primary"
          disabled={busy}
          onclick={() => void run(() => app.client.engines.trustHash(item.id, trustHashes(item)))}
          >{t('engines.trust.accept')}</Button
        >
      </div>
    {/each}

    {#if action}
      <Button
        variant="secondary"
        loading={busy}
        disabled={busy}
        onclick={() => void run(() => app.client.engines.bundleDownload(bundle.id))}
        >{action === 'download'
          ? t('ob.bundle.download', { size: app.i18n.bytes(bundle.missing_bytes) })
          : t(`bundles.${action}`)}</Button
      >
    {/if}
    <p class="muted small">{t('ob.bundle.later')}</p>
    {#if failure}
      <p class="warn" role="alert">{t('ob.local.failed', { error: failure })}</p>
    {/if}
  </section>
{/if}

<style>
  .local {
    display: flex;
    flex-direction: column;
    /* Przyciski mają naturalną szerokość — rozciągnięte na całą kartę wyglądały jak pole. */
    align-items: flex-start;
    gap: var(--alfa-space-2);
    margin-top: var(--alfa-space-4);
  }
  h3 {
    font-size: var(--alfa-font-size-md);
  }
  p {
    margin: 0;
  }
  .name {
    display: flex;
    flex-wrap: wrap;
    align-items: baseline;
    gap: var(--alfa-space-2);
  }
  progress {
    width: 100%;
  }
  .muted {
    color: var(--alfa-color-text-muted);
  }
  .small {
    font-size: var(--alfa-font-size-sm);
  }
  .ok {
    color: var(--alfa-color-success);
  }
  .warn {
    color: var(--alfa-color-warning);
  }
  .trust {
    align-self: stretch;
    display: flex;
    flex-direction: column;
    gap: var(--alfa-space-1);
    padding: var(--alfa-space-3);
    border: 1px solid var(--alfa-color-warning);
    border-radius: var(--alfa-radius-control);
  }
  .hash {
    font-family: var(--alfa-font-mono);
    font-size: var(--alfa-font-size-sm);
    overflow-wrap: anywhere;
    user-select: all;
  }
</style>
