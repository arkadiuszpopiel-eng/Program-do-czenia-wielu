<!--
  „Naucz Alfę” (paleta `Ctrl+K`, panel Pamięć; P5): źródło (tekst / plik / ta rozmowa), zakres
  i tytuł → fakty w pamięci ze źródłem. Tekst i plik to treść z zewnątrz — niezaufana, tylko
  pamięć tej rozmowy (rdzeń odmawia szerszych zakresów). Podgląd liczby faktów liczony jak
  w rdzeniu (`logic/memory-teach`); po zapisie wynik z „Cofnij naukę” i drogą do panelu Pamięć.
-->
<script lang="ts">
  import { Dialog } from 'bits-ui';
  import { Button, SegmentedControl, Select, TextField } from '@alfa/ui-kit';
  import { errorText } from '../../api/command-error';
  import type { RememberScope } from '../../api/types';
  import type { TeachFile, TeachReport, TeachSource } from '../../api/types-memory';
  import {
    chunkText,
    isExternal,
    scopeOfKey,
    teachScopes,
    undoTeach,
  } from '../../logic/memory-teach';
  import { useApp } from '../../state/context';
  import { closeTeach } from '../../state/teach-dialog.svelte';

  type Kind = TeachSource['kind'];

  const app = useApp();
  const { t } = app.i18n;
  const sessionId = $derived(app.activeId);

  let kind = $state<Kind>('text');
  let text = $state('');
  let file = $state<TeachFile | null>(null);
  let scope = $state<RememberScope>('session');
  let title = $state('');
  let busy = $state(false);
  let error = $state<string | null>(null);
  let report = $state<TeachReport | null>(null);
  let note = $state<string | null>(null);

  const external = $derived(isExternal(kind));
  const scopes = $derived(teachScopes(kind));
  const content = $derived(kind === 'text' ? text : kind === 'file' ? (file?.text ?? '') : '');
  const estimate = $derived(external ? chunkText(content) : null);
  const ready = $derived(
    !!sessionId && !busy && (kind === 'conversation' || (estimate?.pieces.length ?? 0) > 0),
  );
  const sourceOptions = $derived(
    (['text', 'file', 'conversation'] as const).map((k) => ({
      value: k,
      label: t(`teach.source.${k}`),
    })),
  );
  const scopeOptions = $derived(scopes.map((s) => ({ value: s, label: t(`teach.scope.${s}`) })));

  function pickKind(next: string) {
    kind = next as Kind;
    error = null;
    if (!teachScopes(kind).includes(scope)) scope = 'session';
  }

  async function pickFile() {
    error = null;
    busy = true;
    try {
      const picked = await app.client.memory.teachPickFile();
      if (picked) file = picked;
    } catch (e) {
      error = errorText(e);
    } finally {
      busy = false;
    }
  }

  function source(): TeachSource {
    if (kind === 'text') return { kind, text };
    if (kind === 'file' && file) return { kind, name: file.name, text: file.text };
    return { kind: 'conversation' };
  }

  async function teach() {
    if (!sessionId || !ready) return;
    error = null;
    busy = true;
    try {
      report = await app.client.memory.teach({
        session_id: sessionId,
        source: source(),
        scope,
        title: title.trim() || null,
      });
    } catch (e) {
      error = errorText(e);
    } finally {
      busy = false;
    }
  }

  async function undo() {
    if (!report) return;
    busy = true;
    const ids = report.ids;
    const result = await undoTeach(ids, (id) =>
      app.client.memory.forget({ target: 'entry', id }),
    );
    busy = false;
    note = result.failed
      ? t('teach.undoFailed', { n: result.failed })
      : t('teach.undone', { n: result.removed });
    report = null;
  }

  function show() {
    closeTeach();
    app.openPanel('memory');
  }
</script>

<Dialog.Root open onOpenChange={(open) => !open && closeTeach()}>
  <Dialog.Portal>
    <Dialog.Overlay class="alfa-teach-overlay" />
    <Dialog.Content class="alfa-teach">
      <Dialog.Title class="alfa-teach-title">{t('teach.title')}</Dialog.Title>
      <Dialog.Description class="alfa-teach-lead">{t('teach.lead')}</Dialog.Description>
      {#if !sessionId}
        <p class="muted">{t('teach.noSession')}</p>
      {:else if report}
        <section class="result" aria-label={t('teach.result')} data-testid="teach-result">
          <p class="strong" role="status">
            {t('teach.done', {
              n: report.facts,
              scope: t(`teach.scope.${scopeOfKey(report.scope_key)}`),
            })}
          </p>
          <p class="muted">{t('teach.doneSource', { source: report.source })}</p>
          {#if !report.trusted}<p class="note">{t('teach.doneUntrusted')}</p>{/if}
          {#if report.truncated}<p class="muted">{t('teach.doneTruncated')}</p>{/if}
          {#if report.duplicates}
            <p class="muted">{t('teach.doneDuplicates', { n: report.duplicates })}</p>
          {/if}
        </section>
      {:else}
        {#if note}<p class="muted" role="status">{note}</p>{/if}
        <SegmentedControl
          label={t('teach.source')}
          value={kind}
          options={sourceOptions}
          onchange={pickKind}
        />
        {#if kind === 'text'}
          <label class="field">
            <span>{t('teach.text')}</span>
            <textarea rows="7" bind:value={text}></textarea>
          </label>
        {:else if kind === 'file'}
          <div class="file">
            <Button variant="secondary" disabled={busy} onclick={() => void pickFile()}
              >{file ? t('teach.pickAgain') : t('teach.pick')}</Button
            >
            {#if file}
              <p class="strong" data-testid="teach-file">
                {t('teach.file', { name: file.name, chars: app.i18n.int(file.chars) })}
              </p>
              {#if file.truncated}<p class="muted">{t('teach.fileTruncated')}</p>{/if}
            {:else}
              <p class="muted">{t('teach.formats')}</p>
            {/if}
          </div>
        {:else}
          <p class="muted">{t('teach.conversation')}</p>
        {/if}
        {#if external}<p class="note">{t('teach.untrusted')}</p>{/if}
        <div class="field">
          <span id="alfa-teach-scope">{t('teach.scope')}</span>
          <Select
            labelledby="alfa-teach-scope"
            size="sm"
            value={scope}
            options={scopeOptions}
            disabled={scopeOptions.length < 2}
            onchange={(v) => (scope = v as RememberScope)}
          />
        </div>
        <TextField label={t('teach.titleField')} hint={t('teach.titleHint')} bind:value={title} />
        {#if estimate && estimate.pieces.length > 0}
          <p class="muted" aria-live="polite" data-testid="teach-estimate">
            {t('teach.estimate', { n: estimate.pieces.length })}{#if estimate.truncated}
              {' '}{t('teach.estimateTruncated')}{/if}
          </p>
        {/if}
      {/if}
      {#if error}<p class="error" role="alert">{error}</p>{/if}
      <div class="actions">
        {#if report}
          <Button variant="ghost" onclick={show}>{t('teach.show')}</Button>
          <Button variant="secondary" disabled={busy} onclick={() => void undo()}
            >{t('teach.undo')}</Button
          >
          <Button variant="primary" onclick={closeTeach}>{t('teach.close')}</Button>
        {:else}
          <Button variant="secondary" onclick={closeTeach}>{t('teach.cancel')}</Button>
          <Button variant="primary" disabled={!ready} loading={busy} onclick={() => void teach()}
            >{t('teach.submit')}</Button
          >
        {/if}
      </div>
    </Dialog.Content>
  </Dialog.Portal>
</Dialog.Root>

<style>
  :global(.alfa-teach-overlay) {
    position: fixed;
    inset: 0;
    z-index: 110;
    background: var(--alfa-color-scrim);
  }
  :global(.alfa-teach) {
    position: fixed;
    z-index: 111;
    top: 50%;
    left: 50%;
    display: flex;
    flex-direction: column;
    gap: var(--alfa-space-3);
    width: min(640px, calc(var(--alfa-vw, 100vw) - 32px));
    max-height: min(calc(var(--alfa-vh, 100vh) * 0.85), 760px);
    padding: var(--alfa-space-6);
    overflow: auto;
    transform: translate(-50%, -50%);
    border: 1px solid var(--alfa-color-border);
    border-radius: var(--alfa-radius-overlay);
    background: var(--alfa-color-surface);
    box-shadow: var(--alfa-shadow-3);
    color: var(--alfa-color-text);
  }
  :global(.alfa-teach-title) {
    margin: 0;
    font-size: var(--alfa-font-size-lg);
    font-weight: var(--alfa-weight-semibold);
  }
  :global(.alfa-teach-lead),
  .muted {
    margin: 0;
    color: var(--alfa-color-text-muted);
    font-size: var(--alfa-font-size-sm);
  }
  .strong {
    margin: 0;
    font-size: var(--alfa-font-size-sm);
    font-weight: var(--alfa-weight-semibold);
    overflow-wrap: anywhere;
  }
  .note {
    margin: 0;
    padding: var(--alfa-space-2);
    border: 1px solid var(--alfa-color-warning);
    border-radius: var(--alfa-radius-control);
    color: var(--alfa-color-text);
    font-size: var(--alfa-font-size-sm);
  }
  .error {
    margin: 0;
    padding: var(--alfa-space-2);
    border: 1px solid var(--alfa-color-error);
    border-radius: var(--alfa-radius-control);
    color: var(--alfa-color-text);
    font-size: var(--alfa-font-size-sm);
  }
  .field {
    display: flex;
    flex-direction: column;
    gap: var(--alfa-space-1);
    font-size: var(--alfa-font-size-sm);
  }
  textarea {
    padding: var(--alfa-space-2);
    border: 1px solid var(--alfa-color-border-strong);
    border-radius: var(--alfa-radius-control);
    background: var(--alfa-color-bg);
    color: var(--alfa-color-text);
    font: inherit;
    resize: vertical;
  }
  .file,
  .result {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: var(--alfa-space-2);
  }
  .actions {
    display: flex;
    flex-wrap: wrap;
    justify-content: flex-end;
    gap: var(--alfa-space-2);
  }
</style>
