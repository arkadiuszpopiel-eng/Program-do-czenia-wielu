<!-- Pliki / artefakty sesji (makieta 8): karta pliku, podgląd tekstu (zwykły tekst), akcje-intencje. -->
<script lang="ts">
  import { Avatar, Button, EmptyState } from '@alfa/ui-kit';
  import FileText from '@lucide/svelte/icons/file-text';
  import FolderOpen from '@lucide/svelte/icons/folder-open';
  import type { ArtifactAction, ArtifactInfo, ArtifactPreview } from '../../api/types';
  import { attempt, load } from '../../state/attempt';
  import { useApp } from '../../state/context';
  import LoadFailed from '../shell/LoadFailed.svelte';

  interface Props {
    sessionId: string;
  }

  let { sessionId }: Props = $props();
  const app = useApp();
  const { t } = app.i18n;
  let files = $state<ArtifactInfo[]>([]);
  let loaded = $state(false);
  let loadError = $state<string | null>(null);
  let selected = $state<string | null>(null);
  let preview = $state<ArtifactPreview | null>(null);
  let previewError = $state<string | null>(null);

  // Błąd listy = „Nie udało się wczytać" z „Ponów", nie mylący pusty stan „brak plików".
  async function loadFiles() {
    const id = sessionId;
    loadError = null;
    const result = await load(() => app.client.files.list(id));
    if (id !== sessionId) return;
    if (result.status === 'ready') {
      files = [...result.value];
      selected = result.value[0]?.id ?? null;
    } else if (result.status === 'failed') {
      files = [];
      loadError = result.error;
    }
    loaded = true;
  }

  async function loadPreview(id: string) {
    preview = null;
    previewError = null;
    const result = await load(() => app.client.files.preview(id));
    if (selected !== id) return;
    if (result.status === 'ready') preview = result.value;
    else if (result.status === 'failed') previewError = result.error;
  }

  async function act(id: string, action: ArtifactAction) {
    const ok = await attempt(app.toasts, () => app.client.files.act(id, action));
    if (ok && action === 'copy') {
      app.toasts.show({ kind: 'success', message: t('common.copied'), timeoutMs: 2500 });
    }
  }

  $effect(() => {
    void loadFiles();
  });

  $effect(() => {
    const id = selected;
    if (id) void loadPreview(id);
    else preview = null;
  });

  const current = $derived(files.find((f) => f.id === selected) ?? null);
  const ACTIONS: readonly {
    action: ArtifactAction;
    key: 'files.open' | 'files.reveal' | 'files.copy' | 'files.saveAs';
  }[] = [
    { action: 'open', key: 'files.open' },
    { action: 'reveal', key: 'files.reveal' },
    { action: 'copy', key: 'files.copy' },
    { action: 'save_as', key: 'files.saveAs' },
  ];
</script>

{#if loadError}
  <LoadFailed error={loadError} onretry={() => void loadFiles()} />
{:else if loaded && files.length === 0}
  <EmptyState title={t('panel.files')} description={t('files.empty')}>
    {#snippet icon()}<FolderOpen size={20} strokeWidth={1.5} />{/snippet}
  </EmptyState>
{:else}
  <div class="files">
    <ul class="list" aria-label={t('files.list')}>
      {#each files as file (file.id)}
        <li>
          <button
            type="button"
            class="file"
            aria-current={file.id === selected ? 'true' : undefined}
            onclick={() => (selected = file.id)}
          >
            <FileText size={16} strokeWidth={1.5} aria-hidden="true" />
            <span class="text">
              <span class="name">{file.name}</span>
              <span class="meta">
                {app.i18n.bytes(file.size_bytes)} · {t('files.versions', { n: file.versions })} · {app.i18n.relative(
                  file.created_at,
                )}
              </span>
            </span>
            {#if file.agent}<Avatar agent={file.agent} size={20} />{/if}
          </button>
        </li>
      {/each}
    </ul>
    {#if current}
      <section class="card" aria-label={t('files.preview', { name: current.name })}>
        <h3 class="card-title">{current.name}</h3>
        <p class="path" data-selectable>{current.path}</p>
        <div class="actions">
          {#each ACTIONS as a (a.action)}
            <Button size="sm" variant="secondary" onclick={() => void act(current.id, a.action)}
              >{t(a.key)}</Button
            >
          {/each}
        </div>
        {#if preview?.kind === 'text'}
          <pre class="preview" data-selectable>{preview.text}</pre>
          {#if preview.truncated}<p class="meta">{t('files.truncated')}</p>{/if}
        {:else if preview?.kind === 'image'}
          <img class="image" src={preview.src} alt={current.name} loading="lazy" />
        {:else if preview}
          <p class="meta">{t('files.noPreview')}</p>
        {:else if previewError}
          <LoadFailed error={previewError} onretry={() => void loadPreview(current.id)} />
        {/if}
      </section>
    {/if}
  </div>
{/if}

<style>
  .files {
    display: flex;
    flex-direction: column;
    gap: var(--alfa-space-3);
  }
  .list {
    display: flex;
    flex-direction: column;
    gap: 2px;
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .file {
    display: flex;
    align-items: center;
    gap: var(--alfa-space-2);
    width: 100%;
    min-height: 40px;
    padding: var(--alfa-space-1) var(--alfa-space-2);
    border: 0;
    border-radius: var(--alfa-radius-control);
    background: transparent;
    color: var(--alfa-color-text);
    text-align: left;
  }
  .file:hover,
  .file[aria-current='true'] {
    background: var(--alfa-color-surface2);
  }
  .text {
    display: flex;
    flex-direction: column;
    flex: 1;
    min-width: 0;
  }
  .name {
    font-size: var(--alfa-font-size-sm);
    font-weight: var(--alfa-weight-semibold);
  }
  .meta,
  .path {
    color: var(--alfa-color-text-muted);
    font-size: var(--alfa-font-size-xs);
  }
  .path {
    overflow-wrap: anywhere;
    font-family: var(--alfa-font-mono);
  }
  .card {
    display: flex;
    flex-direction: column;
    gap: var(--alfa-space-2);
    padding: var(--alfa-space-3);
    border: 1px solid var(--alfa-color-border);
    border-radius: var(--alfa-radius-card);
  }
  .card-title {
    font-size: var(--alfa-font-size-md);
  }
  .actions {
    display: flex;
    flex-wrap: wrap;
    gap: var(--alfa-space-1);
  }
  .preview {
    max-height: 240px;
    margin: 0;
    padding: var(--alfa-space-2);
    overflow: auto;
    border-radius: var(--alfa-radius-control);
    background: var(--alfa-color-surface2);
    white-space: pre-wrap;
  }
  .image {
    max-width: 100%;
    border-radius: var(--alfa-radius-control);
  }
</style>
