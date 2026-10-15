<!--
  Ustawienia → Szablony promptów: lista szablonów ze zmiennymi `{{nazwa}}`, dodawanie, edycja,
  usuwanie z potwierdzeniem i „Użyj” (okno wypełniania i wstawiania do rozmowy).
-->
<script lang="ts">
  import { Button, TextField } from '@alfa/ui-kit';
  import { errorText } from '../../../api/command-error';
  import type { PromptTemplate } from '../../../api/types-templates';
  import LoadFailed from '../../../components/shell/LoadFailed.svelte';
  import Loading from '../../../components/shell/Loading.svelte';
  import { templateVariables } from '../../../logic/templates';
  import { useApp } from '../../../state/context';
  import { openTemplates } from '../../../state/template-dialog.svelte';
  import './work.css';

  const app = useApp();
  const { t } = app.i18n;
  let list = $state<readonly PromptTemplate[] | null>(null);
  let loadError = $state<string | null>(null);
  let editing = $state<number | null>(null);
  let name = $state('');
  let text = $state('');
  let formError = $state<string | null>(null);
  let confirm = $state<string | null>(null);
  let busy = $state(false);
  const vars = $derived(templateVariables(text));

  async function load() {
    try {
      list = await app.client.templates.list();
      loadError = null;
    } catch (e) {
      loadError = errorText(e);
    }
  }

  $effect(() => {
    void load();
  });

  function edit(index: number) {
    const current = index >= 0 ? list?.[index] : undefined;
    name = current?.name ?? '';
    text = current?.text ?? '';
    formError = null;
    editing = index;
  }

  async function store(next: readonly PromptTemplate[]): Promise<boolean> {
    busy = true;
    try {
      list = await app.client.templates.save(next);
      return true;
    } catch (e) {
      if (editing !== null) formError = errorText(e);
      else app.toasts.show({ kind: 'error', message: errorText(e) });
      return false;
    } finally {
      busy = false;
    }
  }

  async function save() {
    if (!list || editing === null) return;
    const next = [...list];
    const item: PromptTemplate = { id: next[editing]?.id ?? '', name, text };
    if (editing < 0) next.push(item);
    else next[editing] = item;
    if (await store(next)) editing = null;
  }

  async function remove(id: string) {
    if (!list) return;
    confirm = null;
    await store(list.filter((x) => x.id !== id));
  }
</script>

<section class="wk-card" aria-labelledby="tpl-title">
  <h3 id="tpl-title">{t('tpl.title')}</h3>
  <p>{t('tpl.intro')}</p>
  {#if loadError}<LoadFailed error={loadError} onretry={() => void load()} />
  {:else if !list}<Loading />
  {:else}
    {#if list.length === 0}<p class="wk-meta">{t('tpl.empty')}</p>{/if}
    <ul class="wk-list" aria-label={t('tpl.list')}>
      {#each list as item, i (item.id)}
        <li>
          <strong>{item.name}</strong>
          {#if templateVariables(item.text).length}
            <span class="wk-meta">{t('tpl.vars')}: {templateVariables(item.text).join(', ')}</span>
          {/if}
          <span class="wk-code">{item.text}</span>
          <div class="wk-actions">
            {#if confirm === item.id}
              <span class="wk-warn">{t('tpl.confirm', { name: item.name })}</span>
              <Button
                size="sm"
                variant="primary"
                disabled={busy}
                onclick={() => void remove(item.id)}>{t('tpl.confirmYes')}</Button
              >
              <Button size="sm" variant="secondary" onclick={() => (confirm = null)}
                >{t('tpl.cancel')}</Button
              >
            {:else}
              <Button size="sm" variant="primary" onclick={() => openTemplates(item.id)}
                >{t('tpl.use')}</Button
              >
              <Button size="sm" variant="secondary" disabled={busy} onclick={() => edit(i)}
                >{t('tpl.edit')}</Button
              >
              <Button size="sm" variant="ghost" disabled={busy} onclick={() => (confirm = item.id)}
                >{t('tpl.remove')}</Button
              >
            {/if}
          </div>
        </li>
      {/each}
    </ul>
    {#if editing === null}
      <div class="wk-actions">
        <Button variant="secondary" disabled={busy || list.length >= 100} onclick={() => edit(-1)}
          >{t('tpl.add')}</Button
        >
      </div>
    {/if}
  {/if}
</section>

{#if editing !== null}
  <section class="wk-card" aria-labelledby="tpl-form">
    <h3 id="tpl-form">{t('tpl.form.title')}</h3>
    <TextField label={t('tpl.form.name')} bind:value={name} maxlength={60} />
    <label class="wk-field">
      <span>{t('tpl.form.text')}</span>
      <textarea rows="6" bind:value={text}></textarea>
    </label>
    <p class="wk-meta">
      {t('tpl.form.hint')}
      {#if vars.length}{t('tpl.vars')}: {vars.join(', ')}{/if}
    </p>
    {#if formError}<p class="wk-error" role="alert">{formError}</p>{/if}
    <div class="wk-actions">
      <Button variant="primary" disabled={busy} onclick={() => void save()}
        >{t('tpl.form.save')}</Button
      >
      <Button variant="secondary" disabled={busy} onclick={() => (editing = null)}
        >{t('tpl.cancel')}</Button
      >
    </div>
  </section>
{/if}
