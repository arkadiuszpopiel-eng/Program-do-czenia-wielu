<!--
  „Wstaw szablon” (paleta `Ctrl+K`, Ustawienia → Szablony promptów): wybór szablonu, pola
  zmiennych `{{nazwa}}`, podgląd i wstawienie do pola wpisywania bieżącej rozmowy (dopisane po
  pustej linii, gdy szkic nie jest pusty). Nic nie jest wysyłane — użytkownik sprawdza i wysyła.
-->
<script lang="ts">
  import { Dialog } from 'bits-ui';
  import { Button, Select } from '@alfa/ui-kit';
  import { errorText } from '../../api/command-error';
  import type { PromptTemplate } from '../../api/types-templates';
  import { appendToDraft, fillTemplate, templateVariables } from '../../logic/templates';
  import { useApp } from '../../state/context';
  import { closeTemplates, templateDialog } from '../../state/template-dialog.svelte';

  const app = useApp();
  const { t } = app.i18n;
  let templates = $state<readonly PromptTemplate[] | null>(null);
  let error = $state<string | null>(null);
  let chosen = $state('');
  let values = $state<Record<string, string>>({});
  const current = $derived(templates?.find((x) => x.id === chosen) ?? null);
  const vars = $derived(current ? templateVariables(current.text) : []);
  const preview = $derived(current ? fillTemplate(current.text, values) : '');
  const options = $derived((templates ?? []).map((x) => ({ value: x.id, label: x.name })));

  $effect(() => {
    app.client.templates.list().then(
      (list) => {
        templates = list;
        chosen = templateDialog.id ?? list[0]?.id ?? '';
      },
      (e: unknown) => (error = errorText(e)),
    );
  });

  function pick(id: string) {
    chosen = id;
    values = {};
  }

  function insert() {
    const id = app.activeId;
    if (!current || !id) return;
    app.setDraft(appendToDraft(app.sessions.drafts[id] ?? '', preview), id);
    closeTemplates();
    if (app.view === 'settings') app.closeSettings();
  }

  function manage() {
    closeTemplates();
    app.openSettings('templates');
  }
</script>

<Dialog.Root open onOpenChange={(open) => !open && closeTemplates()}>
  <Dialog.Portal>
    <Dialog.Overlay class="alfa-tpl-overlay" />
    <Dialog.Content class="alfa-tpl">
      <Dialog.Title class="alfa-tpl-title">{t('tpl.dialog.title')}</Dialog.Title>
      <Dialog.Description class="alfa-tpl-lead">{t('tpl.dialog.lead')}</Dialog.Description>
      {#if error}<p class="wk-error" role="alert">{error}</p>
      {:else if templates && templates.length === 0}
        <p>{t('tpl.dialog.empty')}</p>
      {:else if templates}
        <div class="field">
          <span id="tpl-pick">{t('tpl.dialog.pick')}</span>
          <Select labelledby="tpl-pick" value={chosen} {options} onchange={pick} />
        </div>
        <div class="vars">
          {#each vars as name (name)}
            <label class="field">
              <span>{name}</span>
              <textarea
                rows="2"
                value={values[name] ?? ''}
                oninput={(e) => (values = { ...values, [name]: e.currentTarget.value })}></textarea>
            </label>
          {/each}
        </div>
        <p class="label">{t('tpl.dialog.preview')}</p>
        <pre class="preview" data-testid="tpl-preview">{preview}</pre>
      {/if}
      <div class="actions">
        <Button variant="ghost" onclick={manage}>{t('tpl.dialog.manage')}</Button>
        <Button variant="secondary" onclick={closeTemplates}>{t('tpl.dialog.cancel')}</Button>
        <Button variant="primary" disabled={!current || !app.activeId} onclick={insert}
          >{t('tpl.dialog.insert')}</Button
        >
      </div>
    </Dialog.Content>
  </Dialog.Portal>
</Dialog.Root>

<style>
  :global(.alfa-tpl-overlay) {
    position: fixed;
    inset: 0;
    z-index: 110;
    background: var(--alfa-color-scrim);
  }
  :global(.alfa-tpl) {
    position: fixed;
    z-index: 111;
    top: 50%;
    left: 50%;
    display: flex;
    flex-direction: column;
    gap: var(--alfa-space-3);
    width: min(640px, calc(var(--alfa-vw, 100vw) - 32px));
    max-height: min(calc(var(--alfa-vh, 100vh) * 0.85), 720px);
    padding: var(--alfa-space-6);
    overflow: auto;
    transform: translate(-50%, -50%);
    border: 1px solid var(--alfa-color-border);
    border-radius: var(--alfa-radius-overlay);
    background: var(--alfa-color-surface);
    box-shadow: var(--alfa-shadow-3);
    color: var(--alfa-color-text);
  }
  :global(.alfa-tpl-title) {
    margin: 0;
    font-size: var(--alfa-font-size-lg);
    font-weight: var(--alfa-weight-semibold);
  }
  :global(.alfa-tpl-lead),
  .label {
    margin: 0;
    color: var(--alfa-color-text-muted);
    font-size: var(--alfa-font-size-sm);
  }
  .field {
    display: flex;
    flex-direction: column;
    gap: var(--alfa-space-1);
    font-size: var(--alfa-font-size-sm);
  }
  .vars {
    display: flex;
    flex-direction: column;
    gap: var(--alfa-space-2);
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
  .preview {
    max-height: 200px;
    margin: 0;
    padding: var(--alfa-space-2);
    overflow: auto;
    border: 1px solid var(--alfa-color-border);
    border-radius: var(--alfa-radius-control);
    background: var(--alfa-color-bg);
    font-family: var(--alfa-font-mono);
    font-size: var(--alfa-font-size-xs);
    white-space: pre-wrap;
    overflow-wrap: anywhere;
  }
  .actions {
    display: flex;
    flex-wrap: wrap;
    justify-content: flex-end;
    gap: var(--alfa-space-2);
  }
</style>
