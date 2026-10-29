<!--
  Kanwa artefaktów (C2.8): przełącznik wersji artefaktu — poprzednia / następna i „Wersja N z M”.
  Wersje to kolejne odpowiedzi rozmowy z tym samym artefaktem (`logic/canvas-versions`).
-->
<script lang="ts">
  import { IconButton } from '@alfa/ui-kit';
  import ChevronLeft from '@lucide/svelte/icons/chevron-left';
  import ChevronRight from '@lucide/svelte/icons/chevron-right';
  import type { CanvasItem } from '../../logic/canvas';
  import { useApp } from '../../state/context';

  interface Props {
    versions: readonly CanvasItem[];
    /** Indeks pokazywanej wersji w `versions`. */
    index: number;
    onselect: (version: CanvasItem) => void;
  }

  let { versions, index, onselect }: Props = $props();
  const { t } = useApp().i18n;
  const uid = $props.id();
  const latest = $derived(index === versions.length - 1);

  // Przycisk na końcu listy staje się nieaktywny i traci fokus — fokus przechodzi na drugi.
  function go(step: number) {
    const target = index + step;
    const next = versions[target];
    if (!next) return;
    onselect(next);
    const edge = target === 0 ? 'next' : target === versions.length - 1 ? 'prev' : null;
    if (edge) requestAnimationFrame(() => document.getElementById(`${uid}-${edge}`)?.focus());
  }
</script>

<div class="versions" role="group" aria-label={t('canvas.versions')}>
  <IconButton
    id="{uid}-prev"
    label={t('canvas.versionPrev')}
    size="sm"
    disabled={index === 0}
    onclick={() => go(-1)}
  >
    <ChevronLeft size={16} strokeWidth={1.5} />
  </IconButton>
  <span class="label" aria-live="polite">
    {t('canvas.versionOf', { n: index + 1, total: versions.length })}{latest
      ? ` · ${t('canvas.versionLatest')}`
      : ''}
  </span>
  <IconButton
    id="{uid}-next"
    label={t('canvas.versionNext')}
    size="sm"
    disabled={latest}
    onclick={() => go(1)}
  >
    <ChevronRight size={16} strokeWidth={1.5} />
  </IconButton>
</div>

<style>
  .versions {
    display: flex;
    align-items: center;
    gap: var(--alfa-space-1);
    padding: 0 var(--alfa-space-2) var(--alfa-space-1) var(--alfa-space-3);
  }
  .label {
    color: var(--alfa-color-text-muted);
    font-size: var(--alfa-font-size-xs);
    font-variant-numeric: tabular-nums;
  }
</style>
