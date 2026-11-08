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
  <div class="meter">
    <div class="row">
      <span class="cap">{t('ob.mic.level')}</span>
      {#if micFailed}
        <span class="state warn">{t('ob.mic.failed')}</span>
      {:else}
        <span class="state" class:ok={heard}
          ><span class="dot" aria-hidden="true"></span>{heard
            ? t('ob.mic.ok')
            : t('ob.mic.silent')}</span
        >
      {/if}
    </div>
    <LevelMeter level={app.micLevel} label={t('ob.mic.level')} />
  </div>
{/if}

<style>
  .meter {
    display: flex;
    flex-direction: column;
    gap: var(--alfa-space-3);
    padding: var(--alfa-space-4);
    border: 1px solid var(--alfa-color-border);
    border-radius: var(--alfa-radius-card);
    background: var(--alfa-color-surface2);
  }
  .row {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: space-between;
    gap: var(--alfa-space-2);
  }
  .cap {
    font-size: var(--alfa-font-size-sm);
    font-weight: var(--alfa-weight-semibold);
  }
  .state {
    display: inline-flex;
    align-items: center;
    gap: var(--alfa-space-2);
    color: var(--alfa-color-text-muted);
    font-size: var(--alfa-font-size-sm);
  }
  .dot {
    width: 8px;
    height: 8px;
    border-radius: var(--alfa-radius-full);
    background: currentColor;
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
