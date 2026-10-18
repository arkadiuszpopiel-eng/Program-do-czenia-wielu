<!--
  Karta licencji (zadanie C5.7) w wierszu katalogu „Modele lokalne i silniki”: licencja z identyfikatorem
  i adresem tekstu, autor, użycie komercyjne, wymóg wskazania autora, uwagi (sprawy otwarte, znak wodny
  mowy syntetycznej) i głosy pozycji z ich warunkami. Rozwijana przyciskiem, żeby lista zostawała krótka;
  stan „do potwierdzenia przez człowieka” jest podany słowem, kolor to tylko dodatek.
-->
<script lang="ts">
  import { Button } from '@alfa/ui-kit';
  import type { CommercialUse, LicenseCard } from '../../../api/types-models';
  import { useApp } from '../../../state/context';

  interface Props {
    card: LicenseCard;
    /** Identyfikator pozycji — do unikalnych identyfikatorów w drzewie. */
    itemId: string;
    /** Nazwa pozycji — do etykiety rozwiniętej karty. */
    itemName: string;
  }

  let { card, itemId, itemName }: Props = $props();
  const app = useApp();
  const { t, tk } = app.i18n;
  let open = $state(false);
  const panelId = $derived(`lic-${itemId}`);
  const commercial = (value: CommercialUse) => tk(`engines.license.commercial.${value}`);
</script>

<div class="license">
  <Button
    size="sm"
    variant="ghost"
    aria-expanded={open}
    aria-controls={panelId}
    onclick={() => (open = !open)}
  >
    {t('engines.license.toggle')}
  </Button>
  {#if open}
    <section id={panelId} class="body" aria-label={t('engines.license.region', { name: itemName })}>
      <p class="status" class:warn={!card.confirmed}>
        {t(card.confirmed ? 'engines.license.confirmed' : 'engines.license.unconfirmed')}
      </p>
      <dl>
        <dt>{t('engines.license.name')}</dt>
        <dd>{card.name}</dd>
        <dt>{t('engines.license.id')}</dt>
        <dd class="code" data-selectable>{card.id}</dd>
        <dt>{t('engines.license.url')}</dt>
        <dd class="url" data-selectable>{card.url}</dd>
        <dt>{t('engines.license.author')}</dt>
        <dd>{app.i18n.text(card.author)}</dd>
        <dt>{t('engines.license.commercial')}</dt>
        <dd>{commercial(card.commercial)}</dd>
        <dt>{t('engines.license.attribution')}</dt>
        <dd>
          {t(card.attribution ? 'engines.license.attribution.yes' : 'engines.license.attribution.no')}
        </dd>
        <dt>{t('engines.license.notes')}</dt>
        <dd>{app.i18n.text(card.notes)}</dd>
      </dl>
      {#if card.voices.length > 0}
        <h5>{t('engines.license.voices', { n: card.voices.length })}</h5>
        <ul class="voices">
          {#each card.voices as voice (voice.id)}
            <li>
              <span class="voice-name">{app.i18n.text(voice.name)}</span>
              <span class="meta">
                {t('engines.license.voice.license', { license: voice.license })} ·
                {t('engines.license.voice.commercial', { value: commercial(voice.commercial) })}
              </span>
              <span class="meta">
                {t('engines.license.voice.origin', { origin: app.i18n.text(voice.origin) })}
              </span>
              <span>{app.i18n.text(voice.notes)}</span>
            </li>
          {/each}
        </ul>
      {/if}
      <p class="meta">{t('engines.license.disclaimer')}</p>
    </section>
  {/if}
</div>

<style>
  .license {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: var(--alfa-space-2);
  }
  .body {
    display: flex;
    flex-direction: column;
    gap: var(--alfa-space-2);
    align-self: stretch;
    padding: var(--alfa-space-3);
    border: 1px solid var(--alfa-color-border);
    border-radius: var(--alfa-radius-control);
    background: var(--alfa-color-surface2);
    font-size: var(--alfa-font-size-sm);
  }
  .body p {
    margin: 0;
  }
  .status {
    align-self: flex-start;
    padding: 0 var(--alfa-space-2);
    border: 1px solid var(--alfa-color-success);
    border-radius: var(--alfa-radius-control);
    color: var(--alfa-color-text);
    font-size: var(--alfa-font-size-xs);
  }
  .status.warn {
    border-color: var(--alfa-color-warning);
  }
  dl {
    display: grid;
    grid-template-columns: minmax(110px, max-content) 1fr;
    gap: var(--alfa-space-1) var(--alfa-space-3);
    margin: 0;
  }
  dt {
    color: var(--alfa-color-text-muted);
  }
  dd {
    margin: 0;
  }
  .code {
    font-family: var(--alfa-font-mono);
    overflow-wrap: anywhere;
  }
  .url {
    overflow-wrap: anywhere;
  }
  h5 {
    margin: 0;
    font-size: var(--alfa-font-size-sm);
  }
  .voices {
    display: flex;
    flex-direction: column;
    gap: var(--alfa-space-2);
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .voices li {
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding-left: var(--alfa-space-2);
    border-left: 2px solid var(--alfa-color-border);
  }
  .voice-name {
    font-weight: var(--alfa-weight-semibold);
  }
  .meta {
    color: var(--alfa-color-text-muted);
  }
</style>
