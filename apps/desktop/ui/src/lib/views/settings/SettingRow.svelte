<!-- Jedno ustawienie: etykieta, opis, zakres (globalne / sesja / agentka / maszyna), domyślna, reset. -->
<script lang="ts">
  import { Chip, IconButton, Select, Switch } from '@alfa/ui-kit';
  import RotateCcw from '@lucide/svelte/icons/rotate-ccw';
  import type { SettingDef, SettingValue } from '../../api/types-system';
  import { attempt } from '../../state/attempt';
  import { useApp } from '../../state/context';

  interface Props {
    def: SettingDef;
    /** Nazwa strony (w wynikach wyszukiwania). */
    page?: string;
  }

  let { def, page }: Props = $props();
  const app = useApp();
  const { t } = app.i18n;
  const uid = $props.id();
  const value = $derived<SettingValue>(app.settings[def.key] ?? def.default);
  const label = $derived(app.i18n.text(def.label));
  const isDefault = $derived(value === def.default);

  function show(v: SettingValue): string {
    const c = def.control;
    if (c.kind === 'toggle') return t(v ? 'common.on' : 'common.off');
    if (c.kind === 'select') {
      const option = c.options.find((o) => o.value === v);
      return option ? app.i18n.text(option.label) : String(v);
    }
    if (c.kind === 'number') return `${v}${c.unit ? ` ${c.unit}` : ''}`;
    return String(v);
  }

  /**
   * Zapis z cofnięciem: `setSetting` ustawia wartość optymistycznie, więc po odmowie rdzenia
   * przywracamy poprzednią (kontrolka pokazuje prawdziwy stan) i pokazujemy toast.
   */
  async function save(next: SettingValue) {
    const key = def.key;
    const previous = app.settings[key];
    if (await attempt(app.toasts, () => app.setSetting(key, next))) return;
    if (previous === undefined) delete app.settings[key];
    else app.settings[key] = previous;
  }

  function setNumber(raw: string) {
    if (def.control.kind !== 'number') return;
    const n = Number(raw);
    if (!Number.isFinite(n)) return;
    const { min, max } = def.control;
    void save(Math.min(max, Math.max(min, n)));
  }
</script>

<div class="row" id="setting-{def.key}">
  <div class="text">
    <div class="title">
      <span class="label" id="{uid}-label">{label}</span>
      <Chip size="sm">{t(`settings.scope.${def.scope}`)}</Chip>
      {#if page}<span class="page">{page}</span>{/if}
    </div>
    <p class="desc" id="{uid}-desc">{app.i18n.text(def.description)}</p>
    <p class="meta">
      {t('settings.default', { value: show(def.default) })}
      {#if def.control.kind === 'number'}· {t('settings.range', {
          min: def.control.min,
          max: def.control.max,
        })}{/if}
    </p>
  </div>
  <div class="control">
    {#if def.control.kind === 'toggle'}
      <Switch
        checked={value === true}
        labelledby="{uid}-label"
        describedby="{uid}-desc"
        onchange={(on) => void save(on)}
      />
    {:else if def.control.kind === 'select'}
      <Select
        value={String(value)}
        labelledby="{uid}-label"
        describedby="{uid}-desc"
        options={def.control.options.map((o) => ({
          value: o.value,
          label: app.i18n.text(o.label),
        }))}
        onchange={(v) => void save(v)}
      />
    {:else if def.control.kind === 'number'}
      <span class="number">
        <input
          type="number"
          {value}
          min={def.control.min}
          max={def.control.max}
          step={def.control.step}
          aria-labelledby="{uid}-label"
          aria-describedby="{uid}-desc"
          onchange={(e) => setNumber(e.currentTarget.value)}
        />
        {#if def.control.unit}<span class="unit">{def.control.unit}</span>{/if}
      </span>
    {:else}
      <input
        class="text-input"
        type="text"
        value={String(value)}
        aria-labelledby="{uid}-label"
        aria-describedby="{uid}-desc"
        onchange={(e) => void save(e.currentTarget.value)}
      />
    {/if}
    <IconButton
      label={t('settings.resetLabel', { label })}
      size="sm"
      disabled={isDefault}
      onclick={() => void attempt(app.toasts, () => app.resetSetting(def.key))}
    >
      <RotateCcw size={14} strokeWidth={1.5} />
    </IconButton>
  </div>
</div>

<style>
  .row {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: var(--alfa-space-4);
    padding: var(--alfa-space-3) 0;
    border-bottom: 1px solid var(--alfa-color-border);
    scroll-margin-top: var(--alfa-space-8);
  }
  .row:target {
    outline: 2px solid var(--alfa-color-focus);
    outline-offset: 4px;
    border-radius: 4px;
  }
  .text {
    flex: 1;
    min-width: 0;
  }
  .title {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--alfa-space-2);
  }
  .label {
    font-weight: var(--alfa-weight-semibold);
  }
  .page {
    color: var(--alfa-color-text-subtle);
    font-size: var(--alfa-font-size-xs);
  }
  .desc {
    margin-top: 2px;
    color: var(--alfa-color-text-muted);
    font-size: var(--alfa-font-size-sm);
  }
  .meta {
    margin-top: 2px;
    color: var(--alfa-color-text-subtle);
    font-size: var(--alfa-font-size-xs);
  }
  .control {
    display: flex;
    align-items: center;
    gap: var(--alfa-space-1);
    flex: none;
  }
  .number {
    display: inline-flex;
    align-items: center;
    gap: var(--alfa-space-1);
  }
  input[type='number'],
  .text-input {
    height: var(--alfa-size-control);
    padding: 0 var(--alfa-space-2);
    border: 1px solid var(--alfa-color-border);
    border-radius: var(--alfa-radius-control);
    background: var(--alfa-color-surface);
    color: var(--alfa-color-text);
  }
  input[type='number'] {
    width: 88px;
    text-align: right;
    font-variant-numeric: tabular-nums;
  }
  .text-input {
    width: min(280px, 40vw);
  }
  .unit {
    color: var(--alfa-color-text-muted);
    font-size: var(--alfa-font-size-sm);
  }
  @media (max-width: 720px) {
    .row {
      flex-direction: column;
    }
  }
</style>
