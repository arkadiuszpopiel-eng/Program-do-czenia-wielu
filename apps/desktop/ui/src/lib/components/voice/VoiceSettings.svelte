<!--
  Ustawienia → Głos (F5, ładowane leniwie): słowa wywoławcze (jawne włączenie, pomiar FAR/FRR,
  bramka właściciela), rozpoznawanie głosu (kreator, „wymagaj weryfikacji dla akcji ryzykownych",
  usunięcie profilu), dyktowanie (profile aplikacji) i czytanie (tempo).
-->
<script lang="ts">
  import { useApp } from '../../state/context';
  import LoadFailed from '../shell/LoadFailed.svelte';
  import DictationCard from './DictationCard.svelte';
  import ReadCard from './ReadCard.svelte';
  import SpeakerEnroll from './SpeakerEnroll.svelte';
  import WakeCard from './WakeCard.svelte';
  import { loadVoiceFeatures } from './voice-act';
  import './voice.css';

  const app = useApp();
  const { t } = app.i18n;
  const features = $derived(app.voice.features);
  let loadError = $state<string | null>(null);

  async function loadFeatures() {
    loadError = null;
    loadError = await loadVoiceFeatures(app);
  }

  $effect(() => {
    void loadFeatures();
  });
</script>

{#if loadError}
  <LoadFailed error={loadError} onretry={() => void loadFeatures()} />
{/if}
{#if !features}
  {#if !loadError}<p class="vf-muted">{t('vf.loading')}</p>{/if}
{:else}
  <WakeCard {features} settings />
  <SpeakerEnroll {features} settings />
  <DictationCard {features} settings />
  <ReadCard {features} settings />
{/if}
