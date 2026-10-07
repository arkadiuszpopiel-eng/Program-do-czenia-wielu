<!--
  Wprowadzenie → mikrofon: wybór urządzenia i test poziomu. Komponent istnieje tylko na tym kroku,
  więc test mikrofonu startuje przy wejściu i jest zatrzymywany przy wyjściu. Błąd komendy przy
  liście urządzeń to „Nie udało się wczytać" z „Ponów" — nie mylące „Nie wykryto mikrofonu".
-->
<script lang="ts">
  import { Button, LevelMeter, Select } from '@alfa/ui-kit';
  import type { AudioDevice } from '../../api/types-hub';
  import LoadFailed from '../../components/shell/LoadFailed.svelte';
  import { attempt, load, type Loadable } from '../../state/attempt';
  import { useApp } from '../../state/context';

  const app = useApp();
  const { t } = app.i18n;
  let devices = $state<Loadable<readonly AudioDevice[]>>({ status: 'loading' });
  let device = $state('');
  let heard = $state(false);
  let micFailed = $state(false);

  async function loadDevices() {
    devices = { status: 'loading' };
    const result = await load(() => app.client.voice.devices());
    devices = result;
    if (result.status === 'ready') {
      device = result.value.find((d) => d.default)?.id ?? result.value[0]?.id ?? '';
    }
  }

  $effect(() => {
    void loadDevices();
  });

  $effect(() => {
    app.client.voice
      .startMicTest(device || null)
      .then(() => (micFailed = false))
      .catch(() => (micFailed = true));
    return () => void app.client.voice.stopMicTest().catch(() => undefined);
  });

  $effect(() => {
    if (app.micLevel > 0.2) heard = true;
  });

  function openMicSettings() {
    void attempt(app.toasts, () =>
      app.client.app.openSystemSettings('ms-settings:privacy-microphone'),
    );
  }
</script>

<h2>{t('ob.mic.title')}</h2>
<p class="muted">{t('ob.mic.desc')}</p>
{#if app.system?.mic === 'denied'}
  <p class="warn">{t('banner.micDenied')}</p>
  <Button variant="secondary" onclick={openMicSettings}>{t('banner.micSettings')}</Button>
{:else if devices.status === 'failed'}
  <LoadFailed error={devices.error} onretry={() => void loadDevices()} />
{:else if devices.status === 'loading'}
  <p class="muted" role="status">{t('common.loading')}</p>
{:else if devices.value.length === 0}
  <p class="warn">{t('banner.micMissing')}</p>
{:else}
  <label class="field">
    <span>{t('ob.mic.device')}</span>
    <Select
      bind:value={device}
      options={devices.value.map((d) => ({ value: d.id, label: d.name }))}
      label={t('ob.mic.device')}
    />
  </label>
  <LevelMeter level={app.micLevel} label={t('ob.mic.level')} />
  {#if micFailed}
    <p class="warn">{t('ob.mic.failed')}</p>
  {:else}
    <p class:ok={heard} class="muted">{heard ? t('ob.mic.ok') : t('ob.mic.silent')}</p>
  {/if}
{/if}

<style>
  h2 {
    font-size: var(--alfa-font-size-xl);
  }
  .muted {
    color: var(--alfa-color-text-muted);
  }
  .ok {
    color: var(--alfa-color-success);
    font-weight: var(--alfa-weight-semibold);
  }
  .warn {
    color: var(--alfa-color-warning);
    font-weight: var(--alfa-weight-semibold);
  }
  .field {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: var(--alfa-space-1);
    font-size: var(--alfa-font-size-sm);
    font-weight: var(--alfa-weight-semibold);
  }
</style>
