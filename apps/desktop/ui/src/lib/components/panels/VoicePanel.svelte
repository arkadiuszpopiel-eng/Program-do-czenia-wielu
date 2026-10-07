<!--
  Panel „Głos" (Alt+6, ładowany leniwie): rozmowa głosowa wł./wył., słowa wywoławcze z testem,
  kreator rejestracji głosu, dyktowanie, czytanie na głos i „szybka rozmowa (chmura) — wymaga
  klucza". Zmiany stanu ogłaszane czytnikowi ekranu przez `aria-live` z throttlingiem.
-->
<script lang="ts">
  import { Button } from '@alfa/ui-kit';
  import type { VoiceFeatures } from '../../api/types-voice';
  import { ThrottledAnnouncer, announcement } from '../../logic/voice-features';
  import { load } from '../../state/attempt';
  import { useApp } from '../../state/context';
  import LoadFailed from '../shell/LoadFailed.svelte';
  import DictationCard from '../voice/DictationCard.svelte';
  import ReadCard from '../voice/ReadCard.svelte';
  import SpeakerEnroll from '../voice/SpeakerEnroll.svelte';
  import WakeCard from '../voice/WakeCard.svelte';
  import { loadVoiceFeatures } from '../voice/voice-act';
  import '../voice/voice.css';

  interface Props {
    sessionId: string;
  }

  let { sessionId }: Props = $props();
  const app = useApp();
  const { t } = app.i18n;
  const features = $derived(app.voice.features);
  const status = $derived(app.voice.status);
  const active = $derived(status?.state === 'active');
  let spoken = $state('');
  let prev: VoiceFeatures | null = null;
  const announcer = new ThrottledAnnouncer((text) => (spoken = text));
  let featuresError = $state<string | null>(null);
  let statusError = $state<string | null>(null);

  // Błędy wczytania = „Nie udało się wczytać" z „Ponów" zamiast „Ładowanie…" na zawsze.
  async function loadFeatures() {
    featuresError = null;
    featuresError = await loadVoiceFeatures(app);
  }

  async function loadStatus() {
    statusError = null;
    const result = await load(() => app.client.voice.status());
    if (result.status === 'ready') app.voice.applyStatus(result.value);
    else if (result.status === 'failed') statusError = result.error;
  }

  $effect(() => {
    void sessionId;
    void loadFeatures();
    void loadStatus();
    return () => announcer.dispose();
  });

  $effect(() => {
    const next = features;
    if (!next) return;
    const a = announcement(prev, next);
    prev = next;
    if (a) announcer.push(t(a.key, a.params));
  });

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
</script>

<div class="voice">
  <div class="alfa-visually-hidden" role="status" aria-live="polite" aria-atomic="true">
    {spoken}
  </div>
  <section class="vf-card" aria-labelledby="vp-conv">
    <div class="vf-head">
      <h3 id="vp-conv">{t('vf.conversation')}</h3>
      <Button size="sm" variant="ghost" onclick={() => app.openSettings('voice')}
        >{t('vf.settingsLink')}</Button
      >
    </div>
    {#if statusError}
      <LoadFailed error={statusError} onretry={() => void loadStatus()} />
    {:else if status?.state === 'unavailable'}
      <p class="vf-warn" role="status">
        {status.reason ? app.i18n.text(status.reason) : t('voice.unavailable')}
      </p>
    {:else}
      <div class="vf-row">
        <span class="vf-state">{t(active ? 'voice.active' : 'voice.off')}</span>
        <Button
          size="sm"
          variant={active ? 'secondary' : 'primary'}
          onclick={() => void toggleConversation()}
          >{t(active ? 'voice.stop' : 'voice.start')}</Button
        >
      </div>
    {/if}
  </section>
  {#if featuresError}
    <LoadFailed error={featuresError} onretry={() => void loadFeatures()} />
  {/if}
  {#if !features}
    {#if !featuresError}<p class="vf-muted">{t('vf.loading')}</p>{/if}
  {:else}
    <WakeCard {features} />
    <SpeakerEnroll {features} />
    <DictationCard {features} />
    <ReadCard {features} />
    <section class="vf-card" aria-labelledby="vp-s2s">
      <div class="vf-head">
        <h3 id="vp-s2s">{t('vf.s2s.title')}</h3>
        <span class="vf-badge">{t('vf.s2s.needsKey')}</span>
      </div>
      <p class="vf-muted">{app.i18n.text(features.s2s.reason)}</p>
      <p class="vf-muted">{t('vf.s2s.private')}</p>
    </section>
  {/if}
</div>

<style>
  .voice {
    display: flex;
    flex-direction: column;
    padding: var(--alfa-space-2);
  }
</style>
