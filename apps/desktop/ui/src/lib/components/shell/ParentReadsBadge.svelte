<!--
  Stały znak dla dziecka (decyzja właściciela 3.11, ADR 0029 pkt 2): rodzic włączył PIN-em pełną
  treść rozmów w dzienniku rodzica. Widoczny w belce, dopóki profil Dziecko jest włączony
  i przełącznik treści obowiązuje — dziecko wie, że rodzic może czytać rozmowy.
-->
<script lang="ts">
  import Eye from '@lucide/svelte/icons/eye';
  import { useApp } from '../../state/context';

  const app = useApp();
  const { t } = app.i18n;
  const shown = $derived(app.autonomy.child?.enabled === true && app.autonomy.child.transcripts);
</script>

{#if shown}
  <span
    class="reads"
    role="img"
    aria-label={t('parentLog.readsBadge')}
    title={t('parentLog.readsBadge')}
  >
    <Eye size={14} strokeWidth={2} aria-hidden="true" />
    <span class="word">{t('parentLog.readsWord')}</span>
  </span>
{/if}

<style>
  .reads {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    height: var(--bar-h, 32px);
    padding: 0 var(--alfa-space-3);
    border: 1px solid var(--alfa-color-border-strong);
    border-radius: var(--alfa-radius-full);
    background: var(--alfa-color-surface2);
    color: var(--alfa-color-text);
    font-size: var(--alfa-font-size-xs);
    font-weight: var(--alfa-weight-semibold);
    white-space: nowrap;
  }
  @media (max-width: 1100px) {
    .word {
      display: none;
    }
    .reads {
      width: var(--bar-h, 32px);
      padding: 0;
      justify-content: center;
    }
  }
</style>
