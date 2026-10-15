<!--
  Kopiowanie listy jako JSON do schowka i dodawanie pozycji z wklejonego JSON (szablony promptów,
  własne polecenia). Kształt sprawdza `parse`, zapis i ostateczną walidację — strona i rdzeń.
-->
<script lang="ts" generics="T">
  import { Button } from '@alfa/ui-kit';
  import type { ImportError, ImportResult } from '../../../logic/json-transfer';
  import { useApp } from '../../../state/context';
  import './work.css';

  const {
    count,
    serialize,
    parse,
    onadd,
  }: {
    count: number;
    serialize: () => string;
    parse: (text: string) => ImportResult<T>;
    onadd: (items: T[]) => Promise<boolean>;
  } = $props();
  const app = useApp();
  const { t } = app.i18n;
  let pasting = $state(false);
  let text = $state('');
  let error = $state<string | null>(null);
  let busy = $state(false);

  function describe(e: ImportError): string {
    switch (e.kind) {
      case 'json':
        return t('json.error.json');
      case 'empty':
        return t('json.error.empty');
      case 'tooMany':
        return t('json.error.tooMany', { max: e.max });
      case 'shape':
        return t('json.error.shape', { n: e.index, fields: e.fields });
    }
  }

  async function copy() {
    try {
      await navigator.clipboard.writeText(serialize());
      app.toasts.show({ kind: 'success', message: t('json.copied', { n: count }) });
    } catch {
      app.toasts.show({ kind: 'error', message: t('msg.copyFailed') });
    }
  }

  async function add() {
    const result = parse(text);
    if (!result.ok) {
      error = describe(result.error);
      return;
    }
    busy = true;
    try {
      if (await onadd(result.items)) {
        app.toasts.show({ kind: 'success', message: t('json.added', { n: result.items.length }) });
        pasting = false;
        text = '';
        error = null;
      }
    } finally {
      busy = false;
    }
  }
</script>

<div class="wk-actions">
  <Button size="sm" variant="secondary" disabled={count === 0} onclick={() => void copy()}
    >{t('json.copy')}</Button
  >
  <Button size="sm" variant="secondary" onclick={() => ((pasting = !pasting), (error = null))}
    >{t('json.paste')}</Button
  >
</div>
{#if pasting}
  <label class="wk-field">
    <span>{t('json.field')}</span>
    <textarea rows="6" spellcheck="false" autocomplete="off" bind:value={text}></textarea>
  </label>
  {#if error}<p class="wk-error" role="alert">{error}</p>{/if}
  <div class="wk-actions">
    <Button size="sm" disabled={busy || !text.trim()} onclick={() => void add()}
      >{t('json.add')}</Button
    >
    <Button size="sm" variant="ghost" onclick={() => ((pasting = false), (error = null))}
      >{t('json.cancel')}</Button
    >
  </div>
{/if}
