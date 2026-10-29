<!--
  Kanwa artefaktów (C2.8): zmiany wersji względem poprzedniej — wiersze dodane i usunięte
  z kontekstem, dłuższe ciągi bez zmian zwinięte. Tylko tekst (bez HTML), więc bez sanitizacji.
-->
<script lang="ts">
  import { diffChunks, diffLines, DIFF_MAX_LINES } from '../../logic/canvas-versions';
  import { useApp } from '../../state/context';

  interface Props {
    before: string;
    after: string;
    /** Numer poprzedniej wersji (do opisu). */
    previous: number;
  }

  let { before, after, previous }: Props = $props();
  const { t } = useApp().i18n;
  const diff = $derived.by(() => {
    const lines = diffLines(before, after);
    return lines ? diffChunks(lines) : null;
  });
  const MARK = { add: '+', del: '−', same: ' ' } as const;
</script>

{#if !diff}
  <p class="notice" role="status">{t('canvas.diffTooLarge', { lines: DIFF_MAX_LINES })}</p>
{:else}
  <p class="notice" role="status">
    {diff.added || diff.removed
      ? t('canvas.diffSummary', { added: diff.added, removed: diff.removed, version: previous })
      : t('canvas.diffNone', { version: previous })}
  </p>
  <!-- tabindex: przewijane porównanie dostępne z klawiatury. -->
  <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
  <div class="diff" tabindex="0" role="document" aria-label={t('canvas.changes')}>
    {#each diff.chunks as chunk, i (i)}
      {#if chunk.type === 'skip'}
        <p class="skip">{t('canvas.diffSkipped', { n: chunk.count })}</p>
      {:else}
        {#each chunk.lines as line, j (j)}
          <p class="line {line.op}">
            <span class="mark" aria-hidden="true">{MARK[line.op]}</span><span
              class="alfa-visually-hidden"
              >{line.op === 'same' ? '' : t(`canvas.diff.${line.op}`)}</span
            >{line.text || ' '}
          </p>
        {/each}
      {/if}
    {/each}
  </div>
{/if}

<style>
  .notice {
    margin: 0;
    padding: var(--alfa-space-2) var(--alfa-space-4);
    color: var(--alfa-color-text-muted);
    font-size: var(--alfa-font-size-xs);
  }
  .diff {
    flex: 1;
    min-height: 0;
    padding: var(--alfa-space-2) 0;
    overflow: auto;
    background: var(--alfa-color-surface2);
    font-family: var(--alfa-font-mono);
    font-size: var(--alfa-font-size-xs);
    line-height: 1.5;
  }
  .diff:focus-visible {
    outline: 2px solid var(--alfa-color-focus, var(--alfa-color-accent));
    outline-offset: -2px;
  }
  .line {
    margin: 0;
    /* Wcięcie wiszące: zawinięty wiersz zaczyna się pod tekstem, nie pod znakiem −/+. */
    padding: 0 var(--alfa-space-4) 0 calc(var(--alfa-space-2) + 1.5ch);
    text-indent: -1.5ch;
    white-space: pre-wrap;
    overflow-wrap: anywhere;
  }
  .mark {
    display: inline-block;
    width: 1.5ch;
    color: var(--alfa-color-text-muted);
  }
  .add {
    background: color-mix(in srgb, var(--alfa-color-success) 14%, transparent);
  }
  .del {
    background: color-mix(in srgb, var(--alfa-color-error) 14%, transparent);
  }
  .skip {
    margin: var(--alfa-space-1) 0;
    padding: 0 var(--alfa-space-4);
    color: var(--alfa-color-text-muted);
    font-family: var(--alfa-font-sans);
    font-style: italic;
  }
</style>
