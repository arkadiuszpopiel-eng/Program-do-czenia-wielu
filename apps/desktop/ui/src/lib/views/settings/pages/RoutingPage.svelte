<!--
  Ustawienia → Dobór modelu (zgłoszenie właściciela 2026-10-14): łatwe zadanie — lekki model,
  kod — średni, trudne — mocny; Krytyczka sprawdza na lekkim, odrzucona odpowiedź idzie szczebel
  wyżej. Każda zmiana zapisuje się od razu (`routing_set`) i działa od następnej odpowiedzi.
-->
<script lang="ts">
  import { SegmentedControl, Select, Switch } from '@alfa/ui-kit';
  import type {
    ModelRouting,
    ModelRoutingSettings,
    RoutingLevel,
    RoutingModel,
    RoutingRung,
  } from '../../../api/types-hub';
  import { errorText } from '../../../api/command-error';
  import LoadFailed from '../../../components/shell/LoadFailed.svelte';
  import Loading from '../../../components/shell/Loading.svelte';
  import { LEVELS, RUNGS, STRATEGIES } from '../../../logic/routing';
  import { load } from '../../../state/attempt';
  import { useApp } from '../../../state/context';
  import RoutingPreview from './RoutingPreview.svelte';

  const app = useApp();
  const { t, tk } = app.i18n;
  const uid = $props.id();

  let routing = $state<ModelRouting | null>(null);
  let failed = $state<string | null>(null);
  let saving = $state(false);

  async function reload() {
    const result = await load(() => app.client.accounts.routing());
    if (result.status === 'ready') {
      routing = result.value;
      failed = null;
    } else if (result.status === 'failed') failed = result.error;
  }

  $effect(() => {
    void reload();
  });

  async function save(change: Partial<ModelRoutingSettings>) {
    if (!routing || saving) return;
    saving = true;
    try {
      routing = await app.client.accounts.setRouting({ ...routing.settings, ...change });
    } catch (error) {
      app.toasts.show({ kind: 'error', message: errorText(error) });
    } finally {
      saving = false;
    }
  }

  function setRung(model: RoutingModel, value: string) {
    if (!routing) return;
    const rungs = { ...routing.settings.rungs };
    if (value === 'auto') delete rungs[model.candidate];
    else rungs[model.candidate] = value as RoutingRung;
    void save({ rungs });
  }

  const rungOptions = (model: RoutingModel) => [
    {
      value: 'auto',
      label: t('routing.rung.auto', { rung: tk(`routing.rung.${model.automatic}`) }),
    },
    ...RUNGS.map((r) => ({ value: r, label: tk(`routing.rung.${r}`) })),
  ];
</script>

<p class="lead">{t('routing.intro')}</p>

{#if failed}
  <LoadFailed error={failed} onretry={() => void reload()} />
{:else if !routing}
  <Loading label={t('routing.settings')} />
{:else}
  {@const s = routing.settings}
  <section class="card" aria-labelledby="{uid}-settings">
    <h3 id="{uid}-settings">{t('routing.settings')}</h3>
    <label class="row">
      <Switch
        checked={s.enabled}
        label={t('routing.enabled')}
        disabled={saving}
        onchange={(enabled) => void save({ enabled })}
      />
      <span>
        {t('routing.enabled')}
        <small>{t('routing.enabledHint')}</small>
      </span>
    </label>
    <div class="field">
      <span>{t('routing.strategy')}</span>
      <SegmentedControl
        variant="cards"
        label={t('routing.strategy')}
        value={s.strategy}
        options={STRATEGIES.map((id) => ({
          value: id,
          label: tk(`routing.strategy.${id}`),
          description: tk(`routing.strategy.${id}Hint`),
        }))}
        onchange={(strategy) =>
          void save({ strategy: strategy as ModelRoutingSettings['strategy'] })}
      />
    </div>
    <label class="row">
      <Switch
        checked={s.escalate}
        label={t('routing.escalate')}
        disabled={saving}
        onchange={(escalate) => void save({ escalate })}
      />
      <span>{t('routing.escalate')}</span>
    </label>
    <div class="field">
      <span id="{uid}-review">{t('routing.review')}</span>
      <Select
        value={s.review}
        label={t('routing.review')}
        options={LEVELS.map((l) => ({ value: l, label: tk(`routing.review.${l}`) }))}
        onchange={(review) => void save({ review: review as RoutingLevel })}
      />
    </div>
    <label class="row">
      <Switch
        checked={s.local_first}
        label={t('routing.localFirst')}
        disabled={saving}
        onchange={(local_first) => void save({ local_first })}
      />
      <span>{t('routing.localFirst')}</span>
    </label>
    <label class="row">
      <Switch
        checked={s.reorder_queues}
        label={t('routing.reorder')}
        disabled={saving}
        onchange={(reorder_queues) => void save({ reorder_queues })}
      />
      <span>
        {t('routing.reorder')}
        <small>{t('routing.reorderHint')}</small>
      </span>
    </label>
  </section>

  <section class="card" aria-labelledby="{uid}-models">
    <h3 id="{uid}-models">{t('routing.models')}</h3>
    <p class="muted">{t('routing.modelsHint')}</p>
    {#if routing.models.length === 0}
      <p class="muted">{t('routing.noModels')}</p>
    {:else}
      <ul class="models">
        {#each routing.models as model (model.candidate)}
          <li data-candidate={model.candidate}>
            <span class="who">
              <span class="name">{model.model}</span>
              <span class="muted">
                {[model.provider_name, model.local ? t('routing.local') : '']
                  .filter(Boolean)
                  .join(' · ')}
              </span>
            </span>
            <Select
              value={s.rungs[model.candidate] ?? 'auto'}
              label={t('routing.rungFor', { model: model.model })}
              options={rungOptions(model)}
              onchange={(value) => setRung(model, value)}
            />
          </li>
        {/each}
      </ul>
    {/if}
  </section>

  <RoutingPreview />
{/if}

<style>
  .lead {
    margin: 0 0 var(--alfa-space-4);
    color: var(--alfa-color-text-muted);
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
    margin: 0;
    font-size: var(--alfa-font-size-lg);
  }
  .row {
    display: flex;
    align-items: flex-start;
    gap: var(--alfa-space-3);
    font-size: var(--alfa-font-size-sm);
  }
  .row small {
    display: block;
    margin-top: 2px;
    color: var(--alfa-color-text-muted);
    font-size: var(--alfa-font-size-xs);
  }
  .field {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: var(--alfa-space-1);
    font-size: var(--alfa-font-size-sm);
    font-weight: var(--alfa-weight-semibold);
  }
  .muted {
    margin: 0;
    color: var(--alfa-color-text-muted);
    font-size: var(--alfa-font-size-sm);
  }
  .models {
    display: flex;
    flex-direction: column;
    gap: var(--alfa-space-2);
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .models li {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: space-between;
    gap: var(--alfa-space-2);
    padding: var(--alfa-space-2) var(--alfa-space-3);
    border: 1px solid var(--alfa-color-border);
    border-radius: var(--alfa-radius-control);
  }
  .who {
    display: flex;
    flex-direction: column;
    min-width: 0;
  }
  .name {
    overflow-wrap: anywhere;
    font-family: var(--alfa-font-mono);
    font-size: var(--alfa-font-size-sm);
  }
</style>
