<!--
  Okno „Pipeline'y modeli”: biblioteka łańcuchów, w których kolejne modele współpracują bez
  Twojego pośrednictwa (np. szkic → styl i typografia → architektura komponentów → kod React/Tailwind).
  Stały pasek górny (nazwa, opis, akcje), przewijana lista etapów pośrodku ze strzałkami przepływu
  oraz stały pasek dolny do natychmiastowego uruchomienia pipeline'u w rozmowie.
-->
<script lang="ts">
  import './pipelines-dialog.css';
  import { Dialog } from 'bits-ui';
  import { Button } from '@alfa/ui-kit';
  import { untrack } from 'svelte';
  import { errorText } from '../../api/command-error';
  import type { PipelineDef, PipelineStage } from '../../api/types';
  import type { RoutingModel } from '../../api/types-hub';
  import {
    BUILTIN_NAMES,
    MAX_STAGES,
    blankPipeline,
    blankStage,
    copyOf,
    runMessage,
    validatePipeline,
  } from '../../logic/pipelines';
  import { closePipelines, pipelinesDialog } from '../../state/board-dialog.svelte';
  import { useApp } from '../../state/context';
  import PipelineStages from './PipelineStages.svelte';
  import PipelineLibrary from './PipelineLibrary.svelte';
  import { pipelineDrafts, pipelineDraftKey, pipelineResume } from '../../state/pipeline-drafts';

  const app = useApp();
  const en = $derived(app.i18n.locale === 'en');

  let library = $state<readonly PipelineDef[]>([]);
  let models = $state<readonly RoutingModel[]>([]);
  let loading = $state(true);
  let loadError = $state('');
  let busy = $state(false);
  let draft = $state<PipelineDef>(blankPipeline());
  let original = $state<string | null>(null);
  let snapshot = $state(JSON.stringify(blankPipeline()));
  let runInput = $state('');

  const dirty = $derived(JSON.stringify(draft) !== snapshot);
  const problem = $derived(validatePipeline(draft, en));
  const isBuiltinName = $derived(BUILTIN_NAMES.includes(draft.name.trim().toLowerCase()));
  const toast = (kind: 'success' | 'error' | 'info', message: string): void => {
    app.toasts.show({ kind, message });
  };

  function stash(): void {
    if (loading) return;
    const key = pipelineDraftKey(draft, original);
    pipelineResume.key = key;
    if (dirty || runInput)
      pipelineDrafts.set(key, {
        draft: JSON.parse(JSON.stringify(draft)) as PipelineDef,
        original,
        snapshot,
        input: runInput,
      });
  }
  function close(): void {
    if (!busy) {
      stash();
      closePipelines();
    }
  }
  function open(def: PipelineDef, saved: string | null, restore = true): void {
    const cached = restore ? pipelineDrafts.get(pipelineDraftKey(def, saved)) : undefined;
    draft = JSON.parse(JSON.stringify(cached?.draft ?? def)) as PipelineDef;
    snapshot = cached?.snapshot ?? JSON.stringify(draft);
    original = cached?.original ?? saved;
    if (cached) runInput = cached.input;
  }

  function pick(def: PipelineDef): void {
    stash();
    open(def, def.builtin ? null : def.name);
  }

  function create(): void {
    stash();
    open(blankPipeline(), null, false);
    snapshot = '';
  }

  function duplicate(): void {
    stash();
    open(
      copyOf(
        draft,
        library.map((d) => d.name),
      ),
      null,
      false,
    );
    snapshot = '';
  }

  async function reload(wanted: string | null): Promise<void> {
    loading = true;
    loadError = '';
    const currentDraft = app.activeId ? (app.sessions.drafts[app.activeId] ?? '').trim() : '';
    if (currentDraft && !runInput.trim()) runInput = currentDraft;
    await Promise.all([
      app.client.sessions.pipelinesList(),
      app.client.accounts.routing().then(
        (r) => r.models,
        () => [],
      ),
    ])
      .then(([list, loadedModels]) => {
        library = list;
        models = loadedModels;
        const first =
          (wanted ? list.find((d) => d.name.toLowerCase() === wanted.toLowerCase()) : undefined) ??
          list.find((d) => !d.builtin) ??
          list[0];
        const cached = !wanted ? pipelineDrafts.get(pipelineResume.key) : undefined;
        if (cached) open(cached.draft, cached.original);
        else if (first) pick(first);
      })
      .catch((e) => (loadError = errorText(e)))
      .finally(() => (loading = false));
  }
  $effect(() => {
    const wanted = pipelinesDialog.name;
    untrack(() => void reload(wanted));
  });

  const patchStages = (stages: readonly PipelineStage[]): void => {
    draft = { ...draft, stages };
  };

  function addStage(): void {
    if (draft.stages.length < MAX_STAGES) {
      patchStages([
        ...draft.stages,
        {
          ...blankStage(),
          title: en ? `Stage ${draft.stages.length + 1}` : `Etap ${draft.stages.length + 1}`,
        },
      ]);
    }
  }

  /** Zapisuje własny pipeline (lub nadpisuje wzorzec wbudowany własną wersją). */
  async function save(quiet = false): Promise<string | null> {
    if (problem) return null;
    busy = true;
    try {
      const name = draft.name.trim();
      library = await app.client.sessions.pipelinesSave({ ...draft, builtin: false }, original);
      const saved = library.find((d) => d.name.toLowerCase() === name.toLowerCase());
      pipelineDrafts.delete(pipelineDraftKey(draft, original));
      if (saved) open(saved, saved.name, false);
      if (!quiet)
        toast('success', en ? `Saved pipeline “${name}”.` : `Zapisano pipeline „${name}”.`);
      return name;
    } catch (e) {
      toast('error', errorText(e));
      return null;
    } finally {
      busy = false;
    }
  }

  async function remove(): Promise<void> {
    if (busy || !original) return create();
    busy = true;
    try {
      const targetName = original;
      library = await app.client.sessions.pipelinesRemove(targetName);
      pipelineDrafts.delete(pipelineDraftKey(draft, original));
      pipelineResume.key = '';
      const next = library.find((d) => d.name.toLowerCase() === targetName.toLowerCase());
      if (next) open(next, next.builtin ? null : next.name, false);
      else if (library[0]) open(library[0], library[0].builtin ? null : library[0].name, false);
      else {
        open(blankPipeline(), null, false);
        snapshot = '';
      }
      toast(
        'info',
        isBuiltinName
          ? en
            ? 'Restored built-in template.'
            : 'Przywrócono domyślny wzorzec.'
          : en
            ? 'Pipeline removed.'
            : 'Usunięto pipeline.',
      );
    } catch (e) {
      toast('error', errorText(e));
    } finally {
      busy = false;
    }
  }

  /** Uruchamia pipeline po nazwie w bieżącej rozmowie (albo w nowej). */
  async function run(): Promise<void> {
    if (busy || !runInput.trim() || problem) return;
    const name = dirty ? await save(true) : draft.name.trim();
    if (!name) return;
    const text = runMessage(name, runInput);
    busy = true;
    try {
      if (app.conversation) {
        if (app.activeId && app.sessions.drafts[app.activeId]?.trim() === runInput.trim()) {
          app.setDraft('', app.activeId);
        }
        await app.conversation.send(text, null, null);
      } else {
        const created = await app.client.sessions.create('coding');
        await app.openSession(created.id);
        await app.client.turns.send(created.id, {
          parent_id: null,
          text,
          addressed_to: null,
          profile: null,
        });
      }
      closePipelines();
      if (app.view === 'settings') app.closeSettings();
    } catch (e) {
      toast('error', errorText(e));
    } finally {
      busy = false;
    }
  }
</script>

<Dialog.Root open onOpenChange={(o) => !o && close()}>
  <Dialog.Portal>
    <Dialog.Overlay class="alfa-pipes-overlay" />
    <Dialog.Content class="alfa-pipes" data-testid="pipelines-dialog">
      <header class="top-head">
        <div>
          <Dialog.Title class="alfa-pipes-title">
            {en
              ? 'Model pipelines — multi-model workflows'
              : 'Pipeline’y modeli — łańcuchy współpracy'}
          </Dialog.Title>
          <Dialog.Description class="alfa-pipes-lead">
            {en
              ? 'Assign specific models and roles to each stage: every stage automatically passes its output ({{prev}}) to the next.'
              : 'Przypisz modele i role do kolejnych etapów: wynik każdego etapu ({{prev}}) automatycznie trafia do następnego bez Twojego pośrednictwa.'}
          </Dialog.Description>
        </div>
        <Button size="sm" variant="ghost" disabled={busy} onclick={close}>
          {en ? 'Close' : 'Zamknij'}
        </Button>
      </header>

      {#if loading}
        <p class="muted">…</p>
      {:else if loadError}
        <p role="alert">{loadError}</p>
        <Button onclick={() => void reload(pipelinesDialog.name)}
          >{en ? 'Try again' : 'Ponów'}</Button
        >
      {:else}
        <div class="body" inert={busy}>
          <PipelineLibrary {library} {draft} {original} {en} oncreate={create} onpick={pick} />

          <div class="main">
            <div class="toolbar">
              <div class="meta-grid">
                <label class="meta-field">
                  <span>{en ? 'Pipeline name' : 'Nazwa pipeline’u'}</span>
                  <input
                    type="text"
                    class="ctl"
                    maxlength="60"
                    bind:value={draft.name}
                    data-testid="pipeline-name"
                    placeholder={en ? 'e.g. Sketch → UI code' : 'np. Szkic → kod UI'}
                  />
                </label>
                <label class="meta-field">
                  <span>{en ? 'Description' : 'Opis działania'}</span>
                  <input
                    type="text"
                    class="ctl"
                    maxlength="300"
                    bind:value={draft.description}
                    placeholder={en
                      ? 'What this pipeline does…'
                      : 'Krótki opis do czego służy ten łańcuch…'}
                  />
                </label>
              </div>

              <div class="toolbar-actions">
                <span class="status-hint">
                  {#if problem && dirty}
                    <span class="problem" role="alert">{problem}</span>
                  {:else if draft.builtin && !dirty}
                    {en
                      ? 'Built-in template — edit any stage or model below and click Save to personalise.'
                      : 'Wzorzec domyślny — zmień dowolny model, rolę lub instrukcję poniżej i kliknij „Zapisz”, aby dostosować.'}
                  {:else if dirty}
                    {en ? 'Unsaved changes' : 'Masz niezapisane zmiany'}
                  {:else}
                    {en ? 'Saved in your library' : 'Zapisany w Twojej bibliotece'}
                  {/if}
                </span>
                <div class="btn-group">
                  <Button
                    size="sm"
                    variant="secondary"
                    disabled={draft.stages.length >= MAX_STAGES}
                    onclick={addStage}
                  >
                    {en ? '+ Add stage' : '+ Dodaj etap'} ({draft.stages.length}/{MAX_STAGES})
                  </Button>
                  <Button size="sm" variant="secondary" disabled={busy} onclick={duplicate}>
                    {en ? 'Duplicate' : 'Powiel'}
                  </Button>
                  {#if original}
                    <Button size="sm" variant="ghost" disabled={busy} onclick={() => void remove()}>
                      {isBuiltinName
                        ? en
                          ? 'Restore default'
                          : 'Przywróć domyślny'
                        : en
                          ? 'Delete'
                          : 'Usuń'}
                    </Button>
                  {/if}
                  <Button
                    size="sm"
                    variant="primary"
                    disabled={busy || !!problem || !dirty}
                    data-testid="pipeline-save"
                    onclick={() => void save()}
                  >
                    {en ? 'Save changes' : 'Zapisz'}
                  </Button>
                </div>
              </div>
            </div>

            {#key original ?? draft.builtin}
              <PipelineStages stages={draft.stages} {models} {en} onchange={patchStages} />
            {/key}

            <div class="run-bar">
              <div class="run-input-wrap">
                <input
                  type="text"
                  class="ctl run-input"
                  bind:value={runInput}
                  data-testid="pipeline-input"
                  aria-label={en ? 'Pipeline input' : 'Wejście pipeline’u'}
                  placeholder={en
                    ? 'Enter sketch, description or task to run through this pipeline now…'
                    : 'Wpisz lub wklej szkic / opis zadania, aby od razu uruchomić ten pipeline w rozmowie…'}
                  onkeydown={(e) => {
                    if (e.key === 'Enter') {
                      e.preventDefault();
                      void run();
                    }
                  }}
                />
                <Button
                  variant="primary"
                  disabled={busy || !runInput.trim() || !!problem}
                  data-testid="pipeline-run"
                  onclick={() => void run()}
                >
                  {en ? '▶ Run pipeline in chat' : '▶ Uruchom w rozmowie'}
                </Button>
              </div>
            </div>
          </div>
        </div>
      {/if}
    </Dialog.Content>
  </Dialog.Portal>
</Dialog.Root>
