<!--
  Jeden etap pipeline'u modeli: tytuł, rola, agentka, model (lista Routera + podpowiedzi lub
  dowolny zapis `dostawca:model`) i zadanie z szybkimi przyciskami `{{input}}` / `{{prev}}`.
-->
<script lang="ts">
  import { Button, Select } from '@alfa/ui-kit';
  import type { PipelineStage } from '../../api/types';
  import type { RoutingModel } from '../../api/types-hub';
  import { modelLabel } from '../../logic/arena';
  import {
    MAX_STAGES,
    STAGE_PERSONAS,
    STAGE_ROLES,
    SUGGESTED_MODELS,
    insertGoalToken,
    validModel,
  } from '../../logic/pipelines';

  interface Props {
    stage: PipelineStage;
    index: number;
    total: number;
    models: readonly RoutingModel[];
    en: boolean;
    onchange: (next: PipelineStage) => void;
    onmove: (delta: -1 | 1) => void;
    onduplicate: () => void;
    onremove: () => void;
  }

  const { stage, index, total, models, en, onchange, onmove, onduplicate, onremove }: Props =
    $props();
  const uid = $props.id();
  const modelBad = $derived(!validModel(stage.model));
  const set = (patch: Partial<PipelineStage>): void => onchange({ ...stage, ...patch });

  const allModelChoices = $derived.by(() => {
    const seen = new Set(models.map((m) => m.candidate));
    const extras = SUGGESTED_MODELS.filter((m) => !seen.has(m.candidate));
    return [...models.map((m) => ({ candidate: m.candidate, label: modelLabel(m) })), ...extras];
  });
  const roleOptions = $derived(STAGE_ROLES.map((r) => ({ value: r.id, label: en ? r.en : r.pl })));
  const personaOptions = $derived([
    { value: '', label: en ? 'Auto (by role)' : 'Automatycznie (wg roli)' },
    ...STAGE_PERSONAS.map((p) => ({ value: p.id, label: p.label })),
  ]);
  // Lista podpowiedzi rysowana przez ui-kit (natywny <datalist> rysuje system, poza motywami).
  const modelOptions = $derived([
    { value: '', label: en ? 'Pick from list…' : 'Wybierz z listy…' },
    ...allModelChoices.map((m) => ({ value: m.candidate, label: m.label })),
  ]);
</script>

<section
  class="stage"
  data-testid="pipeline-stage"
  aria-label={`${en ? 'Stage' : 'Etap'} ${index + 1}`}
>
  <header class="stage-head">
    <span class="num">{en ? `Stage ${index + 1}` : `Etap ${index + 1}`}</span>
    <input
      type="text"
      class="ctl title"
      maxlength="60"
      value={stage.title}
      aria-label={en ? 'Stage name' : 'Nazwa etapu'}
      placeholder={en
        ? 'Stage name (e.g. Style & visual guidelines)'
        : 'Nazwa etapu (np. Styl i wytyczne wizualne)'}
      oninput={(e) => set({ title: e.currentTarget.value })}
    />
    <div class="tools">
      <Button
        size="sm"
        variant="ghost"
        disabled={index === 0}
        aria-label={en ? 'Move stage up' : 'Przesuń etap wyżej'}
        onclick={() => onmove(-1)}
      >
        ↑
      </Button>
      <Button
        size="sm"
        variant="ghost"
        disabled={index === total - 1}
        aria-label={en ? 'Move stage down' : 'Przesuń etap niżej'}
        onclick={() => onmove(1)}
      >
        ↓
      </Button>
      <Button size="sm" variant="ghost" disabled={total >= MAX_STAGES} onclick={onduplicate}>
        {en ? 'Duplicate' : 'Powiel'}
      </Button>
      <Button size="sm" variant="ghost" disabled={total <= 1} onclick={onremove}>
        {en ? 'Remove' : 'Usuń'}
      </Button>
    </div>
  </header>

  <div class="grid-row">
    <div class="field">
      <span class="field-label" id="{uid}-role">{en ? 'Role' : 'Rola wykonawcza'}</span>
      <Select
        size="sm"
        labelledby="{uid}-role"
        value={stage.role}
        options={roleOptions}
        onchange={(v) => set({ role: v })}
      />
    </div>

    <div class="field">
      <span class="field-label" id="{uid}-persona">{en ? 'Agent' : 'Agentka obsady'}</span>
      <Select
        size="sm"
        labelledby="{uid}-persona"
        value={stage.persona ?? ''}
        options={personaOptions}
        onchange={(v) => set({ persona: v || null })}
      />
    </div>

    <div class="field">
      <div class="field-head">
        <label class="field-label" for="{uid}-model">
          {en ? 'Stage model (provider:model)' : 'Model etapu (dostawca:model)'}
        </label>
        {#if stage.model}
          <button
            type="button"
            class="tag-btn"
            onclick={() => set({ model: null })}
            title={en ? 'Reset to Router selection' : 'Przywróć automatyczny dobór Routera'}
          >
            {en ? '× Auto (Router)' : '× Auto (Router)'}
          </button>
        {/if}
      </div>
      <input
        id="{uid}-model"
        type="text"
        class="ctl"
        class:bad={modelBad}
        value={stage.model ?? ''}
        placeholder={en
          ? 'Auto (Router) or e.g. openai:gpt-6-astra'
          : 'Auto (Router) lub np. openai:gpt-6-astra'}
        aria-invalid={modelBad}
        oninput={(e) => set({ model: e.currentTarget.value.trim() || null })}
      />
      <Select
        size="sm"
        label={en ? 'Suggested models' : 'Podpowiedzi modeli'}
        value=""
        options={modelOptions}
        onchange={(v) => v && set({ model: v })}
      />
    </div>
  </div>

  <div class="field">
    <div class="field-head">
      <label class="field-label" for="{uid}-goal">
        {en ? 'Stage instructions / prompt' : 'Zadanie i instrukcja dla modelu na tym etapie'}
      </label>
      <div class="token-bar">
        <span class="token-hint">{en ? 'Insert:' : 'Wstaw:'}</span>
        <button
          type="button"
          class="tag-btn"
          onclick={() => set({ goal: insertGoalToken(stage.goal, '{{input}}') })}
          title={en ? 'Insert initial user input' : 'Wstaw pierwotny szkic / wejście użytkownika'}
        >
          + {'{{input}}'}
        </button>
        {#if index > 0}
          <button
            type="button"
            class="tag-btn"
            onclick={() => set({ goal: insertGoalToken(stage.goal, '{{prev}}') })}
            title={en
              ? 'Insert output from previous stage'
              : 'Wstaw wynik wygenerowany przez poprzedni etap'}
          >
            + {'{{prev}}'}
          </button>
        {/if}
      </div>
    </div>
    <textarea
      id="{uid}-goal"
      class="ctl goal"
      rows="3"
      maxlength="4000"
      value={stage.goal}
      aria-label={en ? 'Stage task' : 'Zadanie etapu'}
      oninput={(e) => set({ goal: e.currentTarget.value })}></textarea>
  </div>
</section>

<style>
  .stage {
    display: flex;
    flex-direction: column;
    gap: var(--alfa-space-2);
    padding: var(--alfa-space-3);
    border: 1px solid var(--alfa-color-border);
    border-radius: var(--alfa-radius-card);
    background: var(--alfa-color-bg);
    flex: none;
    min-width: 0;
  }
  .stage-head {
    display: flex;
    align-items: center;
    gap: var(--alfa-space-2);
    flex-wrap: wrap;
  }
  .tools {
    display: flex;
    align-items: center;
    gap: 4px;
    margin-left: auto;
    flex-shrink: 0;
  }
  .num {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    height: 26px;
    padding: 0 var(--alfa-space-2);
    border-radius: var(--alfa-radius-full);
    background: var(--alfa-color-surface2);
    color: var(--alfa-color-text);
    font-size: var(--alfa-font-size-xs);
    font-weight: var(--alfa-weight-semibold);
    white-space: nowrap;
    flex-shrink: 0;
  }
  .grid-row {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: var(--alfa-space-2);
    align-items: end;
  }
  .field {
    display: flex;
    flex-direction: column;
    gap: 4px;
    min-width: 0;
  }
  .grid-row > .field:last-child {
    grid-column: 1 / -1;
  }
  .field-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--alfa-space-2);
    min-height: 18px;
  }
  .field-label {
    color: var(--alfa-color-text-muted);
    font-size: var(--alfa-font-size-xs);
    font-weight: var(--alfa-weight-semibold);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .token-bar {
    display: flex;
    align-items: center;
    gap: 4px;
  }
  .token-hint {
    color: var(--alfa-color-text-subtle);
    font-size: var(--alfa-font-size-xs);
  }
  .tag-btn {
    height: 20px;
    padding: 0 6px;
    border: 1px solid var(--alfa-color-border);
    border-radius: var(--alfa-radius-control);
    background: var(--alfa-color-surface);
    color: var(--alfa-color-text-muted);
    font-size: 11px;
    font-family: var(--alfa-font-mono);
    cursor: pointer;
  }
  .tag-btn:hover {
    background: var(--alfa-color-surface2);
    color: var(--alfa-color-text);
  }
  .ctl {
    box-sizing: border-box;
    width: 100%;
    height: 34px;
    padding: 0 var(--alfa-space-2);
    border: 1px solid var(--alfa-color-border);
    border-radius: var(--alfa-radius-control);
    background: var(--alfa-color-surface);
    color: var(--alfa-color-text);
    font-size: var(--alfa-font-size-sm);
    font-family: inherit;
  }
  .title {
    flex: 1;
    min-width: 140px;
  }
  .goal {
    height: auto;
    min-height: 180px;
    padding: var(--alfa-space-2);
    resize: vertical;
    line-height: 1.45;
  }
  .bad {
    border-color: var(--alfa-color-danger, #c0392b);
  }
  @media (max-width: 720px) {
    .grid-row {
      grid-template-columns: minmax(0, 1fr);
    }
    .tools {
      width: 100%;
      flex-wrap: wrap;
    }
    .field-head {
      flex-wrap: wrap;
    }
  }
</style>
