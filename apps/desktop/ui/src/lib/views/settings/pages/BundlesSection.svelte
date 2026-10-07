<!--
  Ustawienia → Modele i silniki → pakiety 1–6: przewodnik wyboru (zalecenia, „na styk”, agentki,
  naprawa, normy), sprzęt tej maszyny i karty pakietów od 6 (wzorcowy) do 1 (minimalny). Stan na żywo:
  zdarzenia `ModelChanged` / `ModelProgress` → ponowne `models_bundles` (zbite w jedno odświeżenie).
  Pakiet za mocny dla sprzętu — pobranie dopiero po potwierdzeniu; „Napraw” elementu — też.
-->
<script lang="ts">
  import { ConfirmDialog } from '@alfa/ui-kit';
  import { errorText } from '../../../api/command-error';
  import type { DeviceProfile } from '../../../api/types-hub';
  import type { BundleItemView, ModelBundle } from '../../../api/types-models';
  import LoadFailed from '../../../components/shell/LoadFailed.svelte';
  import { primaryGpu, recommendedBundle } from '../../../logic/bundles';
  import { useApp } from '../../../state/context';
  import BundleCard from './BundleCard.svelte';

  interface Props {
    /** Prośba o naprawę elementu (potwierdzenie i wywołanie po stronie strony). */
    onrepair: (item: { id: string; name: string; size_bytes: number }) => void;
  }

  let { onrepair }: Props = $props();
  const app = useApp();
  const { t, tk } = app.i18n;
  let bundles = $state<readonly ModelBundle[] | null>(null);
  let device = $state<DeviceProfile | null>(null);
  let loadError = $state<string | null>(null);
  let busy = $state<string | null>(null);
  let weak = $state<ModelBundle | null>(null);
  let weakOpen = $state(false);
  let refresh: ReturnType<typeof setTimeout> | null = null;

  const recommended = $derived(bundles ? recommendedBundle(bundles) : null);
  const machine = $derived.by(() => {
    if (!device) return null;
    const gpu = primaryGpu(device);
    return t('bundles.machine', {
      gpu: gpu
        ? t('bundles.machine.gpu', { name: gpu.model, vram: app.i18n.bytes(gpu.vram_mb * 2 ** 20) })
        : t('bundles.machine.noGpu'),
      ram: app.i18n.bytes(device.machine.ram_mb * 2 ** 20),
      cores: t('bundles.cores', { n: device.machine.cpu.cores }),
    });
  });

  async function load() {
    try {
      bundles = await app.client.engines.bundles();
      loadError = null;
    } catch (e) {
      loadError = errorText(e);
    }
  }

  /** Sprzęt tylko do opisu — błąd profilu nie blokuje pakietów. */
  async function loadDevice() {
    try {
      device = await app.client.device.profile();
    } catch {
      device = null;
    }
  }

  $effect(() => {
    void load();
    void loadDevice();
    const off = app.on((event) => {
      if (event.type !== 'ModelChanged' && event.type !== 'ModelProgress') return;
      if (refresh !== null) return;
      refresh = setTimeout(() => {
        refresh = null;
        void load();
      }, 300);
    });
    return () => {
      off();
      if (refresh !== null) clearTimeout(refresh);
      refresh = null;
    };
  });

  function replace(bundle: ModelBundle) {
    if (bundles) bundles = bundles.map((b) => (b.id === bundle.id ? bundle : b));
  }

  async function run(
    id: string,
    action: () => Promise<ModelBundle>,
    done: (b: ModelBundle) => void,
  ) {
    busy = id;
    try {
      const bundle = await action();
      replace(bundle);
      done(bundle);
    } catch (e) {
      app.toasts.show({ kind: 'error', message: errorText(e) });
    } finally {
      busy = null;
    }
  }

  /** Pakiet za mocny dla sprzętu — najpierw potwierdzenie (`confirmed` z okna dialogowego). */
  function download(bundle: ModelBundle, confirmed = false) {
    if (bundle.fit.kind === 'too_weak' && !confirmed) {
      weak = bundle;
      weakOpen = true;
      return;
    }
    const name = app.i18n.text(bundle.name);
    void run(
      bundle.id,
      () => app.client.engines.bundleDownload(bundle.id),
      () => app.toasts.show({ kind: 'info', message: t('bundles.started', { name }) }),
    );
  }

  function verify(bundle: ModelBundle) {
    const name = app.i18n.text(bundle.name);
    void run(
      bundle.id,
      () => app.client.engines.bundleVerify(bundle.id),
      (b) =>
        app.toasts.show({
          kind: b.state === 'corrupt' ? 'error' : 'success',
          message: t('bundles.verified', { name, state: tk(`bundles.state.${b.state}`) }),
        }),
    );
  }

  async function itemAction(item: BundleItemView, action: 'download' | 'verify') {
    busy = item.id;
    try {
      if (action === 'download') await app.client.engines.download(item.id);
      else {
        const result = await app.client.engines.verify(item.id);
        const state = tk(`engines.state.${result.state}`);
        app.toasts.show({
          kind: result.state === 'corrupt' ? 'error' : 'success',
          message: t('engines.verified', { name: item.name, state }),
        });
      }
      await load();
    } catch (e) {
      app.toasts.show({ kind: 'error', message: errorText(e) });
    } finally {
      busy = null;
    }
  }
</script>

<section class="wk-card" aria-labelledby="mm-bundles">
  <h3 id="mm-bundles">{t('bundles.title')}</h3>
  <p>{t('bundles.intro')}</p>
  {#if machine}<p class="wk-meta">{machine}</p>{/if}
  <details class="guide">
    <summary>{t('bundles.guide.title')}</summary>
    <ul class="wk-plain">
      <li>{t('bundles.guide.recommended')}</li>
      <li>{t('bundles.guide.tight')}</li>
      <li>{t('bundles.guide.agents')}</li>
      <li>{t('bundles.guide.shared')}</li>
      <li>{t('bundles.guide.repair')}</li>
      <li>{t('bundles.guide.standards')}</li>
    </ul>
  </details>
  {#if loadError}<LoadFailed error={loadError} onretry={() => void load()} />{/if}
  {#if !bundles && !loadError}<p class="wk-meta" role="status">{t('bundles.loading')}</p>{/if}
  {#if bundles}
    <ul class="bundles">
      {#each bundles as bundle (bundle.id)}
        <BundleCard
          {bundle}
          busy={busy !== null}
          ondownload={() => download(bundle)}
          onverify={() => verify(bundle)}
          onitem={(item, action) =>
            action === 'repair' ? onrepair(item) : void itemAction(item, action)}
        />
      {/each}
    </ul>
  {/if}
</section>

<ConfirmDialog
  bind:open={weakOpen}
  title={t('bundles.weak.title', { name: weak ? app.i18n.text(weak.name) : '' })}
  description={t('bundles.weak.body', {
    reason: weak?.fit.reason ? app.i18n.text(weak.fit.reason) : '',
    recommended: recommended
      ? `${recommended.rating} · ${app.i18n.text(recommended.name)}`
      : t('bundles.weak.none'),
  })}
  confirmLabel={t('bundles.weak.confirm')}
  cancelLabel={t('common.cancel')}
  onconfirm={() => {
    weakOpen = false;
    if (weak) download(weak, true);
  }}
/>

<style>
  .guide summary {
    cursor: pointer;
    font-weight: 600;
  }
  .guide ul {
    display: flex;
    flex-direction: column;
    gap: var(--alfa-space-1);
    margin-top: var(--alfa-space-2);
  }
  .bundles {
    display: flex;
    flex-direction: column;
    gap: var(--alfa-space-3);
    margin: 0;
    padding: 0;
    list-style: none;
  }
</style>
