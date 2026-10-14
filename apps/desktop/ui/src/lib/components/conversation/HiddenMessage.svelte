<!--
  Wiadomość ukryta przez właściciela: krótka notka z przyciskiem „Pokaż”. Wydzielone
  z MessageItem.svelte (limit 400 linii na plik).
-->
<script lang="ts">
  import { Button } from '@alfa/ui-kit';
  import { useApp } from '../../state/context';

  interface Props {
    /** Pozycja w liście wiadomości (dla czytnika ekranu). */
    position: number;
    /** Liczba wiadomości w liście. */
    count: number;
    /** Przywraca wiadomość. */
    onunhide: () => void;
  }
  let { position, count, onunhide }: Props = $props();
  const { t } = useApp().i18n;
</script>

<article
  class="hidden-note"
  aria-posinset={position}
  aria-setsize={count}
  aria-label={t('conv.hidden')}
>
  <span>{t('conv.hidden')}</span>
  <Button size="sm" variant="ghost" onclick={onunhide}>{t('conv.unhide')}</Button>
</article>

<style>
  .hidden-note {
    display: flex;
    align-items: center;
    gap: var(--alfa-space-2);
    padding: var(--alfa-space-2) var(--alfa-space-2) var(--alfa-space-2) 40px;
    color: var(--alfa-color-text-subtle);
    font-size: var(--alfa-font-size-xs);
  }
</style>
