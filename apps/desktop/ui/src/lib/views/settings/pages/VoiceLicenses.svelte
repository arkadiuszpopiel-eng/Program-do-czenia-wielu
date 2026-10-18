<!--
  Ustawienia → Głos: karty licencji modeli mowy i ich barw (zadanie C5.7) — te same karty co w wierszach
  katalogu „Modele lokalne i silniki” (`LicenseCardInfo`), tu tylko dla pozycji, które mówią albo mają
  osobne głosy. Błąd wczytania katalogu — komunikat z „Ponów”, nie pusta sekcja.
-->
<script lang="ts">
  import type { ModelItem } from '../../../api/types-models';
  import { load } from '../../../state/attempt';
  import { useApp } from '../../../state/context';
  import LoadFailed from '../../../components/shell/LoadFailed.svelte';
  import LicenseCardInfo from './LicenseCardInfo.svelte';

  const app = useApp();
  const { t } = app.i18n;
  let items = $state<readonly ModelItem[] | null>(null);
  let error = $state<string | null>(null);

  /** Pozycje mowy: modele syntezy i każda pozycja z osobnymi głosami (np. Pocket TTS). */
  const speech = $derived(
    (items ?? []).filter(
      (item) =>
        item.license_card !== null && (item.kind === 'tts' || item.license_card.voices.length > 0),
    ),
  );

  async function loadItems() {
    const result = await load(() => app.client.engines.list());
    if (result.status === 'ready') {
      items = result.value.items;
      error = null;
    } else if (result.status === 'failed') error = result.error;
  }

  $effect(() => {
    void loadItems();
  });
</script>

<section class="card" aria-labelledby="voice-licenses">
  <h3 id="voice-licenses">{t('engines.license.voicePage.title')}</h3>
  <p class="muted">{t('engines.license.voicePage.hint')}</p>
  {#if error}
    <LoadFailed {error} onretry={() => void loadItems()} />
  {:else if items === null}
    <p class="muted" role="status">{t('common.loading')}</p>
  {:else}
    <ul class="list">
      {#each speech as item (item.id)}
        {#if item.license_card}
          <li>
            <span class="name">{item.name}</span>
            <LicenseCardInfo card={item.license_card} itemId={item.id} itemName={item.name} />
          </li>
        {/if}
      {/each}
    </ul>
  {/if}
</section>

<style>
  /* Wygląd jak pozostałe karty strony „Głos” (VoicePage). */
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
  .list {
    display: flex;
    flex-direction: column;
    gap: var(--alfa-space-3);
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .list li {
    display: flex;
    flex-direction: column;
    gap: var(--alfa-space-1);
  }
  .name {
    font-weight: 600;
  }
</style>
