<!--
  Ustawienia → Zaawansowane: surowy edytor plików konfiguracji (wspólny `shared.toml` i nakładka tej
  maszyny). „Sprawdź” liczy klucze albo wskazuje wiersz z błędem; „Zapisz” przechodzi pełną walidację
  rdzenia — odmowa (np. klucz Jądra albo sekret w pliku) zostawia poprzednią treść i podaje powód.
  Pod edytorem zwykłe przełączniki flag eksperymentalnych (z definicji strony).
-->
<script lang="ts">
  import { Button, SegmentedControl } from '@alfa/ui-kit';
  import type { RawLayer } from '../../../api/client-advanced';
  import { errorText } from '../../../api/command-error';
  import type { RawConfig } from '../../../api/types-advanced';
  import LoadFailed from '../../../components/shell/LoadFailed.svelte';
  import Loading from '../../../components/shell/Loading.svelte';
  import { useApp } from '../../../state/context';
  import './work.css';

  const app = useApp();
  const { t } = app.i18n;
  let view = $state<RawConfig | null>(null);
  let loadError = $state<string | null>(null);
  let layer = $state<RawLayer>('shared');
  let drafts = $state<Record<RawLayer, string>>({ shared: '', machine: '' });
  let verdict = $state<{ ok: boolean; text: string } | null>(null);
  let busy = $state(false);
  const saved = $derived(view ? view[layer] : '');
  const dirty = $derived(view !== null && drafts[layer] !== saved);
  const path = $derived(view ? (layer === 'shared' ? view.shared_path : view.machine_path) : '');
  const layers = $derived([
    { value: 'shared', label: t('advanced.layer.shared') },
    { value: 'machine', label: t('advanced.layer.machine') },
  ]);

  function adopt(next: RawConfig) {
    view = next;
    drafts = { shared: next.shared, machine: next.machine };
  }

  async function load() {
    try {
      adopt(await app.client.advanced.raw());
      loadError = null;
    } catch (e) {
      loadError = errorText(e);
    }
  }

  $effect(() => {
    void load();
  });

  async function check() {
    busy = true;
    try {
      const keys = await app.client.advanced.check(drafts[layer]);
      verdict = { ok: true, text: t('advanced.checkOk', { n: keys }) };
    } catch (e) {
      verdict = { ok: false, text: errorText(e) };
    } finally {
      busy = false;
    }
  }

  async function save() {
    busy = true;
    try {
      const next = await app.client.advanced.save(layer, drafts[layer]);
      const other: RawLayer = layer === 'shared' ? 'machine' : 'shared';
      const keep = drafts[other];
      adopt(next);
      drafts = { ...drafts, [other]: keep };
      verdict = { ok: true, text: t('advanced.saved') };
    } catch (e) {
      verdict = { ok: false, text: errorText(e) };
    } finally {
      busy = false;
    }
  }

  function revert() {
    drafts = { ...drafts, [layer]: saved };
    verdict = null;
  }

  function pick(next: string) {
    layer = next === 'machine' ? 'machine' : 'shared';
    verdict = null;
  }
</script>

<section class="wk-card" aria-labelledby="advanced-title">
  <h3 id="advanced-title">{t('advanced.title')}</h3>
  <p>{t('advanced.intro')}</p>
  <p class="wk-meta">{t('advanced.rules')}</p>
  {#if loadError}<LoadFailed error={loadError} onretry={() => void load()} />
  {:else if !view}<Loading />
  {:else}
    {#if view.problems.length}
      <ul class="wk-list" aria-label={t('advanced.problems')}>
        {#each view.problems as problem (problem)}<li class="wk-warn">{problem}</li>{/each}
      </ul>
    {/if}
    <SegmentedControl label={t('advanced.layer')} options={layers} value={layer} onchange={pick} />
    <p class="wk-meta">{t('advanced.file')}: <span class="wk-code">{path}</span></p>
    <label class="wk-field">
      <span
        >{t('advanced.editor', {
          file: layer === 'shared' ? 'shared.toml' : t('advanced.layer.machine'),
        })}</span
      >
      <textarea
        rows="16"
        spellcheck="false"
        autocomplete="off"
        data-testid="advanced-editor"
        bind:value={drafts[layer]}
        oninput={() => (verdict = null)}></textarea>
    </label>
    {#if verdict}
      <p class={verdict.ok ? 'wk-ok' : 'wk-error'} role={verdict.ok ? 'status' : 'alert'}>
        {verdict.text}
      </p>
    {/if}
    <div class="wk-actions">
      <Button variant="secondary" disabled={busy} onclick={() => void check()}
        >{t('advanced.check')}</Button
      >
      <Button variant="primary" disabled={busy || !dirty} onclick={() => void save()}
        >{t('advanced.save')}</Button
      >
      <Button variant="secondary" disabled={busy || !dirty} onclick={revert}
        >{t('advanced.revert')}</Button
      >
      {#if dirty}<span class="wk-meta">{t('advanced.unsaved')}</span>{/if}
    </div>
  {/if}
</section>
