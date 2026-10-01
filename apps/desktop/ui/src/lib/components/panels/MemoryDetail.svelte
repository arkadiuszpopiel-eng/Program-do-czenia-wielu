<!--
  Szczegóły wpisu pamięci: „dlaczego to pamiętam" (powody, źródła, wersje, wywiedzione),
  edycja = nowa wersja, przypięcie, zatwierdzenie propozycji, awans do szerszego zakresu
  i zapomnienie z podglądem kaskady (co zniknie, co wróci, czy baza zostanie zniszczona).
-->
<script lang="ts">
  import { Button, Select } from '@alfa/ui-kit';
  import type {
    MemoryExplanation,
    MemoryForgetPreview,
    MemoryScopeInfo,
    MemoryScopeRef,
  } from '../../api/types-memory';
  import { useApp } from '../../state/context';
  import MemoryJournal from './MemoryJournal.svelte';

  interface Props {
    entryId: string;
    scopes: readonly MemoryScopeInfo[];
    onclose: () => void;
  }

  let { entryId, scopes, onclose }: Props = $props();
  const app = useApp();
  const { t } = app.i18n;
  const RANK = { session: 0, agent: 1, project: 1, global: 2 } as const;
  const REASONS = {
    target: 'memory.reason.target',
    version: 'memory.reason.version',
    derived: 'memory.reason.derived',
  } as const;
  const reasonLabel = (r: string): string =>
    t(REASONS[r as keyof typeof REASONS] ?? REASONS.target);

  let info = $state<MemoryExplanation | null>(null);
  let editing = $state(false);
  let draft = $state('');
  let target = $state('global');
  let preview = $state<MemoryForgetPreview | null>(null);

  async function load() {
    info = await app.client.memory.explain(entryId);
  }

  $effect(() => {
    void entryId;
    preview = null;
    editing = false;
    void load();
  });

  const item = $derived(info?.item ?? null);
  const promoteTargets = $derived.by(
    (): { value: string; label: string; ref: MemoryScopeRef }[] => {
      if (!item) return [];
      const mine = RANK[item.scope.kind];
      const out = scopes
        .filter((s) => RANK[s.scope.kind] > mine)
        .map((s) => ({
          value: s.key,
          label: `${t(`memory.scopeKind.${s.scope.kind}`)}: ${s.label}`,
          ref: s.scope,
        }));
      if (mine < 2 && !out.some((o) => o.value === 'global')) {
        out.push({
          value: 'global',
          label: t('memory.scopeKind.global'),
          ref: { kind: 'global', id: null },
        });
      }
      return out;
    },
  );

  async function act(run: () => Promise<unknown>, message: string) {
    try {
      await run();
      app.toasts.show({ kind: 'success', message });
      await load();
    } catch (error) {
      app.toasts.show({
        kind: 'error',
        message: String(error instanceof Error ? error.message : error),
      });
    }
  }

  async function save() {
    const text = draft.trim();
    if (!text) return;
    await act(
      () => app.client.memory.edit(entryId, { text, subject: null, confidence: null }),
      t('memory.saved'),
    );
    editing = false;
  }

  async function promote() {
    const ref = promoteTargets.find((p) => p.value === target)?.ref;
    if (!ref) return;
    const label = promoteTargets.find((p) => p.value === target)?.label ?? target;
    await act(
      () => app.client.memory.promote(entryId, ref),
      t('memory.promoted', { scope: label }),
    );
  }

  async function forget() {
    const report = await app.client.memory.forget({ target: 'entry', id: entryId });
    app.toasts.show({
      kind: 'success',
      message: t('memory.forgotten', {
        removed: report.removed,
        versions: report.versions,
        derived: report.derived,
      }),
    });
    onclose();
  }

  function source(): string {
    if (!item) return '';
    if (item.source === 'agent')
      return t('memory.source.agent', { agent: item.source_detail ?? '' });
    if (item.source === 'untrusted')
      return t('memory.source.untrusted', { detail: item.source_detail ?? '' });
    return t(item.source === 'import' ? 'memory.source.import' : 'memory.source.user');
  }
</script>

{#if info && item}
  <section class="detail" aria-label={t('memory.details')}>
    <h4>{t('memory.why')}</h4>
    <ul class="reasons">
      {#each info.reasons as reason (reason)}<li>{reason}</li>{/each}
    </ul>
    <p class="meta" data-selectable>
      {source()} · {t('memory.confidence', { value: app.i18n.percent(item.confidence) })} · {t(
        'memory.created',
        { when: app.i18n.relative(item.created_at) },
      )}{#if item.expires_at}
        · {t('memory.expires', { when: app.i18n.relative(item.expires_at) })}{/if}
    </p>
    {#if info.sources.length}
      <h4>{t('memory.sources')}</h4>
      <ul class="plain">
        {#each info.sources as s (s.id)}
          <li class="mono">{s.id}{s.exists ? '' : ` (${t('memory.sourceMissing')})`}</li>
        {/each}
      </ul>
    {/if}
    {#if info.versions.length > 1}
      <h4>{t('memory.versions')}</h4>
      <ol class="plain">
        {#each info.versions as v (v.id)}
          <li class:current={v.id === item.id}>
            <span class="meta">{t('memory.version', { n: v.version })}</span>
            {v.text}
          </li>
        {/each}
      </ol>
    {/if}
    {#if info.derived.length}
      <p class="meta">{t('memory.derived')}: {info.derived.length}</p>
    {/if}

    {#if editing}
      <label class="edit">
        <span>{t('memory.editLabel')}</span>
        <textarea rows="3" bind:value={draft}></textarea>
      </label>
      <div class="actions">
        <Button size="sm" onclick={save}>{t('memory.save')}</Button>
        <Button size="sm" variant="ghost" onclick={() => (editing = false)}
          >{t('memory.cancel')}</Button
        >
      </div>
    {:else if preview}
      <div class="forget" role="group" aria-label={t('memory.forgetTitle')}>
        <h4>{t('memory.forgetCount', { n: preview.remove.length })}</h4>
        <ul class="plain">
          {#each preview.remove as r (r.id)}
            <li><span class="tag">{reasonLabel(r.reason)}</span> {r.text}</li>
          {/each}
        </ul>
        {#if preview.revive.length}<p class="meta">
            {t('memory.revive', { n: preview.revive.length })}
          </p>{/if}
        {#if preview.shred}<p class="meta">{t('memory.shred')}</p>{/if}
        <div class="actions">
          <Button size="sm" variant="danger" onclick={forget}>{t('memory.forgetConfirm')}</Button>
          <Button size="sm" variant="ghost" onclick={() => (preview = null)}
            >{t('memory.cancel')}</Button
          >
        </div>
      </div>
    {:else}
      <div class="actions">
        {#if item.state === 'pending'}
          <Button
            size="sm"
            onclick={() => act(() => app.client.memory.approve(entryId), t('memory.approved'))}
            >{t('memory.approve')}</Button
          >
        {/if}
        <Button
          size="sm"
          variant="secondary"
          onclick={() => {
            draft = item.text;
            editing = true;
          }}>{t('memory.edit')}</Button
        >
        <Button
          size="sm"
          variant="secondary"
          onclick={() =>
            act(
              () => app.client.memory.setPinned(entryId, !item.pinned),
              t(item.pinned ? 'memory.unpin' : 'memory.pin'),
            )}>{t(item.pinned ? 'memory.unpin' : 'memory.pin')}</Button
        >
        <Button
          size="sm"
          variant="danger"
          onclick={async () =>
            (preview = await app.client.memory.forgetPreview({ target: 'entry', id: entryId }))}
          >{t('memory.forget')}</Button
        >
      </div>
      {#if promoteTargets.length}
        <div class="promote">
          <Select
            label={t('memory.promoteTo')}
            size="sm"
            bind:value={target}
            options={promoteTargets.map(({ value, label }) => ({ value, label }))}
          />
          <Button size="sm" variant="secondary" onclick={promote}>{t('memory.promote')}</Button>
        </div>
      {/if}
    {/if}
    <MemoryJournal scope={item.scope_key} onchange={load} />
  </section>
{/if}

<style>
  .detail {
    display: flex;
    flex-direction: column;
    gap: var(--alfa-space-2);
    margin: var(--alfa-space-1) 0 var(--alfa-space-2);
    padding: var(--alfa-space-3);
    border: 1px solid var(--alfa-color-border);
    border-radius: var(--alfa-radius-card);
    background: var(--alfa-color-surface);
    font-size: var(--alfa-font-size-sm);
  }
  h4 {
    font-size: var(--alfa-font-size-sm);
  }
  .reasons,
  .plain {
    margin: 0;
    padding-left: var(--alfa-space-4);
  }
  .current {
    font-weight: var(--alfa-weight-semibold);
  }
  .meta {
    color: var(--alfa-color-text-muted);
    font-size: var(--alfa-font-size-xs);
  }
  .mono {
    font-family: var(--alfa-font-mono);
    font-size: var(--alfa-font-size-xs);
    overflow-wrap: anywhere;
  }
  .actions,
  .promote {
    display: flex;
    flex-wrap: wrap;
    align-items: flex-end;
    gap: var(--alfa-space-2);
  }
  .edit {
    display: flex;
    flex-direction: column;
    gap: var(--alfa-space-1);
  }
  textarea {
    width: 100%;
    padding: var(--alfa-space-2);
    border: 1px solid var(--alfa-color-border-strong);
    border-radius: var(--alfa-radius-control);
    background: var(--alfa-color-bg);
    color: var(--alfa-color-text);
    font: inherit;
    resize: vertical;
  }
  .forget {
    display: flex;
    flex-direction: column;
    gap: var(--alfa-space-2);
    padding: var(--alfa-space-2);
    border: 1px solid var(--alfa-color-error);
    border-radius: var(--alfa-radius-control);
  }
  .tag {
    padding: 0 var(--alfa-space-1);
    border: 1px solid var(--alfa-color-border);
    border-radius: var(--alfa-radius-control);
    font-size: var(--alfa-font-size-xs);
  }
</style>
