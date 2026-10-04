<!--
  Ustawienia → Głos (F5, ładowane leniwie): słowa wywoławcze (jawne włączenie, pomiar FAR/FRR,
  bramka właściciela), rozpoznawanie głosu (kreator, „wymagaj weryfikacji dla akcji ryzykownych",
  usunięcie profilu), dyktowanie (profile aplikacji) i czytanie (tempo).
-->
<script lang="ts">
  import { useApp } from '../../state/context';
  import DictationCard from './DictationCard.svelte';
  import ReadCard from './ReadCard.svelte';
  import SpeakerEnroll from './SpeakerEnroll.svelte';
  import WakeCard from './WakeCard.svelte';
  import './voice.css';

  const app = useApp();
  const { t } = app.i18n;
  const features = $derived(app.voice.features);

  $effect(() => {
    void app.client.voiceFeatures.features().then((f) => app.voice.applyFeatures(f));
  });
</script>

{#if !features}
  <p class="vf-muted">{t('vf.loading')}</p>
{:else}
  <WakeCard {features} settings />
  <SpeakerEnroll {features} settings />
  <DictationCard {features} settings />
  <ReadCard {features} settings />
{/if}
