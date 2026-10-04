<!--
  Ustawienia → Głos: dostępność trybu głosowego (bez modeli/sidecarów — powód i lista braków),
  rozmowa wł./wył., mikrofon z testem poziomu, głosy agentek v0 z próbką. Tryb PTT/przełącznik
  i pigułka — wiersze ustawień pod spodem (z manifestu strony). F5 (słowa wywoławcze, weryfikacja
  głosu, dyktowanie, czytanie) — sekcje ładowane leniwie (`VoiceSettings`).
-->
<script lang="ts">
  import { Avatar, Button, Select, agentIds, agents } from '@alfa/ui-kit';
  import type { AudioDevice } from '../../../api/types-hub';
  import { useApp } from '../../../state/context';
  import Lazy from '../../../components/shell/Lazy.svelte';

  const app = useApp();
  const { t } = app.i18n;
  let devices = $state<AudioDevice[]>([]);
  let device = $state('');
  let testing = $state(false);

  const status = $derived(app.voice.status);
  const unavailable = $derived(status?.state === 'unavailable');
  const active = $derived(status?.state === 'active');
  const level = $derived(Math.round(app.micLevel * 100));

  $effect(() => {
    void app.client.voice.status().then((s) => app.voice.applyStatus(s));
    void app.client.voice.devices().then((list) => {
      devices = [...list];
      device = list.find((d) => d.default)?.id ?? list[0]?.id ?? '';
    });
    return () => {
      if (testing) void app.client.voice.stopMicTest();
    };
  });

  async function toggleTest() {
    try {
      if (testing) await app.client.voice.stopMicTest();
      else await app.client.voice.startMicTest(device || null);
      testing = !testing;
    } catch (error) {
      app.toasts.show({
        kind: 'error',
        message: error instanceof Error ? error.message : String(error),
      });
    }
  }

  async function toggleConversation() {
    try {
      await app.client.voice.setMicEnabled(!active);
    } catch (error) {
      app.toasts.show({
        kind: 'warning',
        message: error instanceof Error ? error.message : String(error),
      });
    }
  }

  async function preview(agent: (typeof agentIds)[number]) {
    try {
      await app.client.voice.preview(agent);
    } catch (error) {
      app.toasts.show({
        kind: 'warning',
        message: error instanceof Error ? error.message : String(error),
      });
    }
  }
</script>

<section class="card" aria-labelledby="voice-state">
  <h3 id="voice-state">{t('voice.state')}</h3>
  {#if unavailable}
    <p class="warn" role="status">
      {status?.reason ? app.i18n.text(status.reason) : t('voice.unavailable')}
    </p>
    {#if status && status.missing.length}
      <p class="muted">{t('voice.missing')}</p>
      <ul class="list">
        {#each status.missing as item (item)}<li>{item}</li>{/each}
      </ul>
      <p class="muted">{t('voice.modelsHint')}</p>
    {/if}
  {:else}
    <p role="status">{t(active ? 'voice.active' : 'voice.off')}</p>
    <div class="row">
      <Button size="sm" variant={active ? 'secondary' : 'primary'} onclick={toggleConversation}
        >{t(active ? 'voice.stop' : 'voice.start')}</Button
      >
    </div>
  {/if}
</section>

<section class="card" aria-labelledby="voice-mic">
  <h3 id="voice-mic">{t('voice.mic')}</h3>
  {#if devices.length === 0}
    <p class="muted">{t('voice.noDevices')}</p>
  {:else}
    <label class="field">
      <span>{t('voice.device')}</span>
      <Select
        bind:value={device}
        size="sm"
        label={t('voice.device')}
        options={devices.map((d) => ({ value: d.id, label: d.name }))}
      />
    </label>
    <div class="row">
      <Button size="sm" variant="secondary" onclick={toggleTest}
        >{t(testing ? 'voice.testStop' : 'voice.testStart')}</Button
      >
      <div
        class="meter"
        role="meter"
        aria-label={t('voice.level')}
        aria-valuemin={0}
        aria-valuemax={100}
        aria-valuenow={testing ? level : 0}
      >
        <span class="bar" style:transform="scaleX({testing ? app.micLevel : 0})"></span>
      </div>
    </div>
  {/if}
</section>

<Lazy
  load={() => import('../../../components/voice/VoiceSettings.svelte')}
  props={{}}
  label={t('vf.loading')}
/>

<section class="card" aria-labelledby="voice-voices">
  <h3 id="voice-voices">{t('voice.voices')}</h3>
  <p class="muted">{t('voice.voicesHint')}</p>
  <ul class="voices">
    {#each agentIds as id (id)}
      <li class="voice">
        <Avatar agent={id} size={24} decorative />
        <span class="name">{agents[id].name}</span>
        <span class="muted">{t(`voice.bible.${id}`)}</span>
        <Button size="sm" variant="ghost" onclick={() => void preview(id)}
          >{t('voice.preview', { name: agents[id].name })}</Button
        >
      </li>
    {/each}
  </ul>
</section>

<style>
  .card {
    display: flex;
    flex-direction: column;
    gap: var(--alfa-space-2);
    margin-bottom: var(--alfa-space-4);
    padding: var(--alfa-space-4);
    border: 1px solid var(--alfa-color-border);
    border-radius: var(--alfa-radius-card);
    background: var(--alfa-color-surface);
    font-size: var(--alfa-font-size-sm);
  }
  h3 {
    font-size: var(--alfa-font-size-md);
  }
  .muted {
    color: var(--alfa-color-text-muted);
    font-size: var(--alfa-font-size-xs);
  }
  .warn {
    color: var(--alfa-color-text);
    font-weight: 600;
  }
  .list {
    margin: 0;
    padding-left: var(--alfa-space-4);
  }
  .row {
    display: flex;
    align-items: center;
    gap: var(--alfa-space-3);
  }
  .field {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .meter {
    flex: 1;
    height: 8px;
    overflow: hidden;
    border-radius: var(--alfa-radius-full);
    background: var(--alfa-color-surface2);
  }
  .bar {
    display: block;
    height: 100%;
    background: var(--alfa-color-info);
    transform-origin: left;
  }
  .voices {
    display: flex;
    flex-direction: column;
    gap: var(--alfa-space-1);
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .voice {
    display: flex;
    align-items: center;
    gap: var(--alfa-space-2);
  }
  .name {
    min-width: 4em;
    font-weight: 600;
  }
  .voice .muted {
    flex: 1;
  }
</style>
