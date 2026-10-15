<!--
  Karta modelu (AI Act art. 50, zadanie C2.9) w wierszu katalogu „Modele lokalne i silniki”:
  pochodzenie, przeznaczenie, ograniczenia, licencja i strona źródła. Rozwijana, żeby lista
  zostawała krótka; to, czy pozycja tworzy treść AI, widać na znaczku obok (w `ModelRow`).
-->
<script lang="ts">
  import type { ModelCard } from '../../../api/types-models';
  import { useApp } from '../../../state/context';

  interface Props {
    card: ModelCard;
    license: string;
    source: string;
  }

  let { card, license, source }: Props = $props();
  const app = useApp();
  const { t } = app.i18n;
</script>

<details class="model-card">
  <summary>{t('engines.card')}</summary>
  <dl>
    <dt>{t('engines.card.origin')}</dt>
    <dd>{app.i18n.text(card.origin)}</dd>
    <dt>{t('engines.card.purpose')}</dt>
    <dd>{app.i18n.text(card.purpose)}</dd>
    <dt>{t('engines.card.limits')}</dt>
    <dd>{app.i18n.text(card.limits)}</dd>
    <dt>{t('engines.card.output')}</dt>
    <dd>{t(card.generative ? 'engines.card.generative' : 'engines.card.notGenerative')}</dd>
    <dt>{t('engines.card.license')}</dt>
    <dd>{license}</dd>
    <dt>{t('engines.card.source')}</dt>
    <dd class="source">{source}</dd>
  </dl>
</details>

<style>
  summary {
    cursor: pointer;
    font-weight: 600;
    font-size: var(--alfa-font-size-sm);
  }
  dl {
    display: grid;
    grid-template-columns: minmax(110px, max-content) 1fr;
    gap: var(--alfa-space-1) var(--alfa-space-3);
    margin: var(--alfa-space-2) 0 0;
    font-size: var(--alfa-font-size-sm);
  }
  dt {
    color: var(--alfa-color-text-muted);
  }
  dd {
    margin: 0;
  }
  .source {
    overflow-wrap: anywhere;
  }
</style>
