<!--
  Załączniki wysłanej wiadomości (artefakty sesji): rodzaj, nazwa, rozmiar; kliknięcie otwiera
  panel Pliki (podgląd, Otwórz, Pokaż w Eksploratorze — PLAN §11).
-->
<script lang="ts">
  import FileIcon from '@lucide/svelte/icons/file';
  import FileText from '@lucide/svelte/icons/file-text';
  import ImageIcon from '@lucide/svelte/icons/image';
  import type { TurnAttachment } from '../../api/types-files';
  import { useApp } from '../../state/context';

  interface Props {
    attachments: readonly TurnAttachment[];
  }

  let { attachments }: Props = $props();
  const app = useApp();
  const { t } = app.i18n;
</script>

<ul class="attachments" aria-label={t('att.inMessage')}>
  {#each attachments as a, i (`${a.artifact_id ?? a.name}-${i}`)}
    <li>
      <button
        type="button"
        class="att"
        aria-label={t('att.open', { name: a.name })}
        onclick={() => app.openPanel('files')}
      >
        <span class="icon" aria-hidden="true">
          {#if a.kind === 'image'}<ImageIcon size={14} strokeWidth={1.5} />
          {:else if a.kind === 'text'}<FileText size={14} strokeWidth={1.5} />
          {:else}<FileIcon size={14} strokeWidth={1.5} />{/if}
        </span>
        <span class="name">{a.name}</span>
        {#if a.bytes !== null}<span class="size">{app.i18n.bytes(a.bytes)}</span>{/if}
      </button>
    </li>
  {/each}
</ul>

<style>
  .attachments {
    display: flex;
    flex-wrap: wrap;
    gap: var(--alfa-space-1);
    margin: var(--alfa-space-1) 0 0;
    padding: 0;
    list-style: none;
  }
  .att {
    display: inline-flex;
    align-items: center;
    gap: var(--alfa-space-1);
    min-height: 28px;
    max-width: 260px;
    padding: 2px var(--alfa-space-2);
    border: 1px solid var(--alfa-color-border);
    border-radius: var(--alfa-radius-control);
    background: var(--alfa-color-surface);
    color: var(--alfa-color-text);
    font-size: var(--alfa-font-size-xs);
    cursor: pointer;
  }
  .att:hover {
    background: var(--alfa-color-surface2);
  }
  .att:focus-visible {
    outline: var(--alfa-size-focus-ring) solid var(--alfa-color-focus);
    outline-offset: 1px;
  }
  .icon {
    display: inline-flex;
    color: var(--alfa-color-text-muted);
  }
  .name {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .size {
    color: var(--alfa-color-text-muted);
  }
</style>
