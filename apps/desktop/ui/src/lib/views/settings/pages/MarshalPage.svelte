<!--
  Ustawienia → Reguły Marszałka: polecenie (albo szkice JSON z edytora) → propozycja z podglądem
  zawężenia (kiedy, co zawęża, odrzucone szkice, konflikty) → zatwierdź / odrzuć. Reguły tylko
  zawężają; zatwierdza wyłącznie użytkownik. Obowiązujące reguły z cofnięciem, polityka
  efektywna i raport dnia.
-->
<script lang="ts">
  import { Button, TextField } from '@alfa/ui-kit';
  import type { MarshalProposalInfo, MarshalState } from '../../../api/types-tasks';
  import LoadFailed from '../../../components/shell/LoadFailed.svelte';
  import { attempt, load } from '../../../state/attempt';
  import { useApp } from '../../../state/context';

  const app = useApp();
  const { t, tk } = app.i18n;
  let info = $state<MarshalState | null>(null);
  let text = $state('');
  let editor = $state('');
  let editorError = $state<string | undefined>(undefined);
  let current = $state<MarshalProposalInfo | null>(null);
  let report = $state<string | null>(null);
  let reporting = $state(false);
  let loadError = $state<string | null>(null);

  async function reload() {
    const result = await load(() => app.client.marshal.state());
    if (result.status === 'ready') {
      info = result.value;
      loadError = null;
    } else if (result.status === 'failed') loadError = result.error;
  }

  $effect(() => {
    void reload();
  });

  async function run(action: () => Promise<unknown>, message?: string) {
    if (!(await attempt(app.toasts, action))) return;
    if (message) app.toasts.show({ kind: 'success', message });
    await reload();
  }

  async function showReport() {
    reporting = true;
    await attempt(app.toasts, async () => {
      report = (await app.client.marshal.report()).text;
    });
    reporting = false;
  }

  async function propose() {
    editorError = undefined;
    let drafts: unknown[] | null = null;
    if (editor.trim()) {
      try {
        const parsed: unknown = JSON.parse(editor);
        if (!Array.isArray(parsed)) throw new Error('not array');
        drafts = parsed;
      } catch {
        editorError = t('marshal.editorInvalid');
        return;
      }
    }
    await run(async () => {
      current = await app.client.marshal.propose(text.trim(), drafts);
    });
  }

  async function decide(approve: boolean) {
    const p = current;
    if (!p) return;
    await run(async () => {
      if (approve) {
        const rules = await app.client.marshal.approve(p.id);
        app.toasts.show({ kind: 'success', message: t('marshal.approved', { n: rules.length }) });
      } else await app.client.marshal.reject(p.id);
      current = null;
    });
  }
</script>

<section class="card" aria-labelledby="ms-title">
  <h3 id="ms-title">{t('marshal.title')}</h3>
  <p class="desc">{t('marshal.intro')}</p>
  {#if info && !info.translator}<p class="note">{t('marshal.noTranslator')}</p>{/if}
  <form
    class="form"
    onsubmit={(e) => {
      e.preventDefault();
      void propose();
    }}
  >
    <TextField label={t('marshal.command')} bind:value={text} />
    <label class="editor">
      <span>{t('marshal.editor')}</span>
      <textarea
        rows="4"
        spellcheck="false"
        bind:value={editor}
        aria-invalid={editorError ? 'true' : undefined}></textarea>
      {#if editorError}<span class="error" role="alert">{editorError}</span>{/if}
    </label>
    <div>
      <Button type="submit" disabled={!text.trim() && !editor.trim()}>{t('marshal.propose')}</Button
      >
    </div>
  </form>
</section>

{#if current}
  <section class="card proposal" aria-labelledby="ms-proposal">
    <h3 id="ms-proposal">{t('marshal.proposal', { text: current.text })}</h3>
    <h4>{t('marshal.narrowing')}</h4>
    <ul class="rules">
      {#each current.rules as rule (rule.id)}
        <li>
          <strong>{rule.description}</strong>
          <span class="meta">{t('marshal.when')}: {rule.when.join(', ')}</span>
          <span>{t('marshal.effects')}: {rule.effects.join('; ')}</span>
        </li>
      {/each}
    </ul>
    {#if current.rejected.length}
      <h4>{t('marshal.rejected')}</h4>
      <ul class="rules">
        {#each current.rejected as r, i (i)}<li class="error">{r.errors.join('; ')}</li>{/each}
      </ul>
    {/if}
    {#if current.conflicts.length}
      <h4>{t('marshal.conflicts')}</h4>
      <ul class="rules">
        {#each current.conflicts as c (c)}<li>{c}</li>{/each}
      </ul>
    {/if}
    <div class="actions">
      <Button onclick={() => decide(true)} disabled={current.rules.length === 0}
        >{t('marshal.approve')}</Button
      >
      <Button variant="secondary" onclick={() => decide(false)}>{t('marshal.reject')}</Button>
    </div>
  </section>
{/if}

{#if loadError}
  <LoadFailed error={loadError} onretry={() => void reload()} />
{/if}
{#if info}
  <section class="card" aria-labelledby="ms-rules">
    <h3 id="ms-rules">{t('marshal.rules')}</h3>
    {#if info.rules.length === 0}
      <p class="meta">{t('marshal.rulesEmpty')}</p>
    {:else}
      <ul class="rules">
        {#each info.rules as rule (rule.id)}
          <li>
            <strong>{rule.description}</strong>
            <span class="meta">{t('marshal.when')}: {rule.when.join(', ')}</span>
            <span>{t('marshal.effects')}: {rule.effects.join('; ')}</span>
            <div>
              <Button
                size="sm"
                variant="ghost"
                aria-label={`${t('marshal.revoke')}: ${rule.description}`}
                onclick={() => run(() => app.client.marshal.revoke(rule.id), t('marshal.revoked'))}
                >{t('marshal.revoke')}</Button
              >
            </div>
          </li>
        {/each}
      </ul>
    {/if}
    <h4>{t('marshal.effective')}</h4>
    <ul class="plain">
      {#each info.effective as line (line)}<li>{line}</li>{/each}
    </ul>
    {#if info.proposals.length}
      <ul class="plain meta">
        {#each info.proposals.slice(0, 5) as p (p.id)}
          <li>„{p.text}" — {tk(`marshal.status.${p.status}`)}</li>
        {/each}
      </ul>
    {/if}
    <div class="actions">
      <Button
        size="sm"
        variant="secondary"
        loading={reporting}
        disabled={reporting}
        onclick={showReport}>{t('marshal.report')}</Button
      >
    </div>
    {#if report}<p class="report" role="status">{report}</p>{/if}
  </section>
{/if}

<style>
  .card {
    display: flex;
    flex-direction: column;
    gap: var(--alfa-space-3);
    margin-bottom: var(--alfa-space-4);
    padding: var(--alfa-space-4);
    border: 1px solid var(--alfa-color-border);
    border-radius: var(--alfa-radius-card);
    background: var(--alfa-color-surface);
    font-size: var(--alfa-font-size-sm);
  }
  .proposal {
    border-color: var(--alfa-color-info);
  }
  h3 {
    font-size: var(--alfa-font-size-md);
  }
  h4 {
    font-size: var(--alfa-font-size-sm);
  }
  .form,
  .editor {
    display: flex;
    flex-direction: column;
    gap: var(--alfa-space-2);
  }
  textarea {
    width: 100%;
    padding: var(--alfa-space-2);
    border: 1px solid var(--alfa-color-border-strong);
    border-radius: var(--alfa-radius-control);
    background: var(--alfa-color-bg);
    color: var(--alfa-color-text);
    font-family: var(--alfa-font-mono);
    font-size: var(--alfa-font-size-xs);
    resize: vertical;
  }
  .rules {
    display: flex;
    flex-direction: column;
    gap: var(--alfa-space-2);
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .rules li {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .plain {
    margin: 0;
    padding-left: var(--alfa-space-4);
  }
  .meta {
    color: var(--alfa-color-text-muted);
    font-size: var(--alfa-font-size-xs);
  }
  .note {
    font-size: var(--alfa-font-size-xs);
    font-style: italic;
  }
  .error {
    color: var(--alfa-color-error);
    font-size: var(--alfa-font-size-xs);
  }
  .actions {
    display: flex;
    gap: var(--alfa-space-2);
  }
</style>
