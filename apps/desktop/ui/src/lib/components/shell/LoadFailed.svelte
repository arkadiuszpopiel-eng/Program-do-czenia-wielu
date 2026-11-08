<!--
  Stan „nie udało się wczytać" z komunikatem rdzenia i „Ponów" — zamiast pustej listy, mylącego
  pustego stanu albo „Ładowanie…" na zawsze (lib/state/attempt.ts, `Loadable`).
-->
<script lang="ts">
  import { Button } from '@alfa/ui-kit';
  import { useApp } from '../../state/context';

  interface Props {
    error: string;
    onretry: () => void;
  }

  let { error, onretry }: Props = $props();
  const { t } = useApp().i18n;
</script>

<div class="failed" role="alert">
  <p>{t('common.loadFailed', { error })}</p>
  <Button size="sm" variant="secondary" onclick={onretry}>{t('common.retry')}</Button>
</div>

<style>
  .failed {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: space-between;
    gap: var(--alfa-space-2);
    padding: var(--alfa-space-2) var(--alfa-space-3);
    border: 1px solid var(--alfa-color-error);
    border-radius: var(--alfa-radius-control);
    color: var(--alfa-color-text);
    font-size: var(--alfa-font-size-sm);
  }
  p {
    margin: 0;
  }
</style>
