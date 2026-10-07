<!--
  Ustawienia → Kreator agentek: rozmowa (opis → szkic + pytania) albo formularz → podgląd persony
  (rdzeń: odmiana imienia, kolor, głos v0, rola, narzędzia, limity) → test na sucho → zapis.
  Zapis tylko po zaliczonym teście tego samego hasha; autonomia ≤ sufit z Brokera (nigdy L4
  bez przełącznika w Ustawieniach). Biblioteka agentek utworzonych w Kreatorze.
-->
<script lang="ts">
  import { Button } from '@alfa/ui-kit';
  import type {
    AgentDraft,
    BuilderAgentInfo,
    BuilderDryRun,
    BuilderPolicyView,
    BuilderPreview as Preview,
  } from '../../../api/types-work';
  import { errorText } from '../../../api/command-error';
  import LoadFailed from '../../../components/shell/LoadFailed.svelte';
  import Loading from '../../../components/shell/Loading.svelte';
  import { emptyDraft } from '../../../logic/work';
  import { load } from '../../../state/attempt';
  import { useApp } from '../../../state/context';
  import BuilderForm from './BuilderForm.svelte';
  import BuilderPreview from './BuilderPreview.svelte';
  import './work.css';

  const app = useApp();
  const { t } = app.i18n;
  let policy = $state<BuilderPolicyView | null>(null);
  let library = $state<BuilderAgentInfo[]>([]);
  let description = $state('');
  let questions = $state<readonly string[]>([]);
  let initial = $state.raw<AgentDraft>(emptyDraft());
  let generation = $state(0);
  let draft = $state.raw<AgentDraft>(emptyDraft());
  let preview = $state<Preview | null>(null);
  let dry = $state<BuilderDryRun | null>(null);
  let error = $state<string | null>(null);
  let busy = $state(false);
  let loaded = $state(false);
  let loadError = $state<string | null>(null);

  /** Polityka (bez niej nie ma formularza) i biblioteka; błąd → komunikat z „Ponów". */
  async function reload() {
    const result = await load(() =>
      Promise.all([app.client.builder.policy(), app.client.builder.library()]),
    );
    if (result.status === 'ready') {
      const [p, l] = result.value;
      policy = p;
      library = [...l];
      loaded = true;
      loadError = null;
    } else if (result.status === 'failed') loadError = result.error;
  }

  $effect(() => {
    void reload();
  });

  async function step<T>(action: () => Promise<T>): Promise<T | null> {
    busy = true;
    error = null;
    try {
      return await action();
    } catch (e) {
      error = errorText(e);
      return null;
    } finally {
      busy = false;
    }
  }

  async function propose() {
    const proposal = await step(() => app.client.builder.propose(description.trim()));
    if (!proposal) return;
    questions = proposal.questions;
    initial = proposal.draft;
    draft = proposal.draft;
    generation++;
    preview = null;
    dry = null;
  }

  async function showPreview() {
    const p = await step(() => app.client.builder.preview(draft));
    if (p) preview = p;
  }

  async function dryRun() {
    const result = await step(() => app.client.builder.dryRun(draft));
    if (result) dry = result;
  }

  async function save() {
    const p = preview;
    if (!p || !dry || dry.hash !== p.hash) return;
    const saved = await step(() => app.client.builder.save(draft, p.hash));
    if (!saved) return;
    app.toasts.show({ kind: 'success', message: t('builder.saved', { name: p.name }) });
    initial = emptyDraft();
    draft = initial;
    generation++;
    preview = null;
    dry = null;
    description = '';
    questions = [];
    await reload();
  }

  async function voice() {
    await step(() => app.client.builder.voicePreview(draft));
  }
</script>

<section class="wk-card" aria-labelledby="bd-title">
  <h3 id="bd-title">{t('builder.title')}</h3>
  <p>{t('builder.intro')}</p>
  <label class="wk-field">
    <span>{t('builder.describe')}</span>
    <textarea rows="3" bind:value={description} placeholder={t('builder.describeHint')}></textarea>
  </label>
  <div class="wk-actions">
    <Button size="sm" variant="secondary" disabled={busy || !description.trim()} onclick={propose}
      >{t('builder.propose')}</Button
    >
  </div>
  {#if questions.length}
    <ul class="wk-plain">
      {#each questions as q (q)}<li>{q}</li>{/each}
    </ul>
  {/if}
  <!-- Bez polityki nie ma formularza niżej — błąd „Zaproponuj" musi być widoczny tutaj. -->
  {#if error && !policy}<p class="wk-error" role="alert">{error}</p>{/if}
</section>

{#if loadError}
  <LoadFailed error={loadError} onretry={() => void reload()} />
{:else if !loaded}
  <Loading />
{/if}

{#if policy}
  <section class="wk-card" aria-labelledby="bd-form">
    <h3 id="bd-form">{t('builder.form')}</h3>
    {#key generation}
      <BuilderForm
        {initial}
        {policy}
        onchange={(d) => {
          draft = d;
          if (preview) preview = null;
        }}
      />
    {/key}
    {#if error}<p class="wk-error" role="alert">{error}</p>{/if}
    <div class="wk-actions">
      <Button size="sm" variant="primary" disabled={busy} onclick={showPreview}
        >{t('builder.showPreview')}</Button
      >
    </div>
  </section>
{/if}

{#if preview}
  <BuilderPreview {preview} {dry} {busy} ondry={dryRun} onsave={save} onvoice={voice} />
{/if}

<section class="wk-card" aria-labelledby="bd-library">
  <h3 id="bd-library">{t('builder.library')}</h3>
  {#if loaded && library.length === 0}
    <p class="wk-meta">{t('builder.libraryEmpty')}</p>
  {:else if library.length}
    <ul class="wk-list">
      {#each library as a (a.persona)}
        <li>
          <strong>{a.name}</strong>
          <span class="wk-meta">{a.role} · {a.autonomy} · {a.color}</span>
        </li>
      {/each}
    </ul>
  {/if}
</section>
