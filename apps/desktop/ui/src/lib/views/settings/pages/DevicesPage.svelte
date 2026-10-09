<!-- Urządzenia: profil sprzętu tej maszyny i zalecenia (dane z device-profile). -->
<script lang="ts">
  import { Button } from '@alfa/ui-kit';
  import type { DeviceProfile } from '../../../api/types-hub';
  import LoadFailed from '../../../components/shell/LoadFailed.svelte';
  import Loading from '../../../components/shell/Loading.svelte';
  import { load, showError } from '../../../state/attempt';
  import { useApp } from '../../../state/context';

  const app = useApp();
  const { t, tk } = app.i18n;
  let profile = $state<DeviceProfile | null>(null);
  let loadError = $state<string | null>(null);
  let measuring = $state(false);

  async function reload() {
    const result = await load(() => app.client.device.profile());
    if (result.status === 'ready') {
      profile = result.value;
      loadError = null;
    } else if (result.status === 'failed') loadError = result.error;
  }

  $effect(() => {
    void reload();
  });

  async function measure() {
    measuring = true;
    try {
      profile = await app.client.device.measure();
    } catch (error) {
      showError(app.toasts, error);
    } finally {
      measuring = false;
    }
  }
</script>

{#if loadError && !profile}
  <LoadFailed error={loadError} onretry={() => void reload()} />
{:else if !profile}
  <Loading />
{/if}

{#if profile}
  {@const m = profile.machine}
  <section class="card" aria-labelledby="dev-machine">
    <h3 id="dev-machine">{t('dev.machine')}: {m.name}</h3>
    <dl class="grid">
      <dt>{t('dev.os')}</dt>
      <dd>{m.os}</dd>
      <dt>{t('dev.cpu')}</dt>
      <dd>
        {m.cpu.model} · {t('dev.cores', { n: m.cpu.cores })} · {t('dev.threads', {
          n: m.cpu.threads,
        })}
      </dd>
      <dt>{t('dev.ram')}</dt>
      <dd>{app.i18n.bytes(m.ram_mb * 1024 * 1024)}</dd>
      <dt>{t('dev.gpu')}</dt>
      <dd>
        <ul class="gpus">
          {#each m.gpus as gpu, index (index)}
            <li>
              <span class="gpu-name">{gpu.vendor} {gpu.model}</span>
              <span class="gpu-role {gpu.role}">{tk(`dev.gpuRole.${gpu.role}`)}</span>
              <span class="gpu-meta">
                {gpu.integrated
                  ? t('dev.gpuShared', { size: app.i18n.bytes(gpu.shared_mb * 1024 * 1024) })
                  : app.i18n.bytes(gpu.vram_mb * 1024 * 1024)} · {gpu.backends.join(', ')}
              </span>
              <span class="gpu-hint">{tk(`dev.gpuHint.${gpu.role}`)}</span>
            </li>
          {:else}
            <li>{t('common.none')}</li>
          {/each}
        </ul>
      </dd>
      <dt>{t('dev.npu')}</dt>
      <dd>{m.npu ?? t('common.none')}</dd>
      <dt>{t('dev.battery')}</dt>
      <dd>
        {#if m.battery}{app.i18n.percent(m.battery.percent / 100)} · {t(
            m.battery.on_ac ? 'dev.onAc' : 'dev.onBattery',
          )}{:else}{t('common.none')}{/if}
      </dd>
    </dl>
  </section>
  <section class="card" aria-labelledby="dev-rec">
    <h3 id="dev-rec">{t('dev.recommendation')}</h3>
    <dl class="grid">
      <dt>{t('dev.hwClass')}</dt>
      <dd>{t(`dev.hw.${profile.recommendation.hw_class}`)}</dd>
      <dt>{t('dev.voiceProfile')}</dt>
      <dd>{profile.recommendation.voice_profile}</dd>
      <dt>{t('dev.llm')}</dt>
      <dd>{profile.recommendation.llm_backend}</dd>
    </dl>
    <h4>{t('dev.tradeoffs')}</h4>
    <ul class="list">
      {#each profile.recommendation.tradeoffs as item (item.pl)}<li>
          {app.i18n.text(item)}
        </li>{/each}
    </ul>
    <div class="foot">
      <span class="muted"
        >{t('dev.measured', { when: app.i18n.relative(profile.measured_at) })}</span
      >
      <Button size="sm" variant="secondary" loading={measuring} onclick={measure}
        >{t('dev.measure')}</Button
      >
    </div>
  </section>
{/if}

<style>
  .gpus {
    display: flex;
    flex-direction: column;
    gap: var(--alfa-space-3);
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .gpus li {
    display: grid;
    grid-template-columns: auto 1fr;
    gap: 2px var(--alfa-space-2);
    align-items: center;
  }
  .gpu-name {
    font-weight: var(--alfa-weight-semibold);
  }
  .gpu-role {
    justify-self: start;
    padding: 0 var(--alfa-space-2);
    border: 1px solid var(--alfa-color-border-strong);
    border-radius: var(--alfa-radius-full);
    font-size: var(--alfa-font-size-xs);
    line-height: 1.6;
  }
  .gpu-role.spare {
    border-color: var(--alfa-color-border);
    color: var(--alfa-color-text-muted);
  }
  .gpu-meta,
  .gpu-hint {
    grid-column: 1 / -1;
    color: var(--alfa-color-text-muted);
    font-size: var(--alfa-font-size-sm);
  }
  .card {
    display: flex;
    flex-direction: column;
    gap: var(--alfa-space-3);
    margin-bottom: var(--alfa-space-4);
    padding: var(--alfa-space-4);
    border: 1px solid var(--alfa-color-border);
    border-radius: var(--alfa-radius-card);
    background: var(--alfa-color-surface);
  }
  h3 {
    font-size: var(--alfa-font-size-md);
  }
  h4 {
    font-size: var(--alfa-font-size-sm);
  }
  .grid {
    display: grid;
    grid-template-columns: max-content 1fr;
    gap: var(--alfa-space-1) var(--alfa-space-4);
    margin: 0;
    font-size: var(--alfa-font-size-sm);
  }
  dt {
    color: var(--alfa-color-text-muted);
  }
  dd {
    margin: 0;
  }
  .list {
    margin: 0;
    padding-left: var(--alfa-space-4);
    font-size: var(--alfa-font-size-sm);
  }
  .foot {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--alfa-space-2);
  }
  .muted {
    color: var(--alfa-color-text-muted);
    font-size: var(--alfa-font-size-xs);
  }
</style>
