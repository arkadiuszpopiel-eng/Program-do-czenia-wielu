<!--
  Stan „trwa wczytywanie” widoku czekającego na rdzeń: szkielet dopiero po 300 ms (szybka odpowiedź
  nie miga), z opisem dla czytników ekranu (role=status w Skeleton) — zamiast pustej karty
  (PLAN §14.8; audyt `e2e/slow-core.spec.ts`).
-->
<script lang="ts">
  import { Skeleton } from '@alfa/ui-kit';
  import { useApp } from '../../state/context';

  interface Props {
    /** Opis dla czytników ekranu (domyślnie „Ładowanie…”). */
    label?: string;
    lines?: number;
  }

  let { label, lines = 3 }: Props = $props();
  const { t } = useApp().i18n;
  let slow = $state(false);

  $effect(() => {
    const handle = setTimeout(() => (slow = true), 300);
    return () => clearTimeout(handle);
  });
</script>

{#if slow}
  <div class="loading"><Skeleton {lines} label={label ?? t('common.loading')} /></div>
{/if}

<style>
  .loading {
    padding: var(--alfa-space-2) 0;
  }
</style>
