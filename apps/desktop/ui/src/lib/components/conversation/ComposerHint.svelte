<!--
  Podpowiedź pod polem pisania: informacja o rozmowie z AI (AI Act) i klawisze wysyłania, a gdy
  wpisana wiadomość dostanie odpowiedź bez modelu (polecenie wbudowane, własne albo quiz) —
  dopisek „Bez modelu: …” (tester rdzenia `quick_commands_test`, z opóźnieniem 300 ms).
-->
<script lang="ts">
  import { useApp } from '../../state/context';

  const {
    draft,
    sendOnEnter,
    attachments,
  }: { draft: string; sendOnEnter: boolean; attachments: number } = $props();
  const app = useApp();
  const { t } = app.i18n;
  let local = $state<string | null>(null);
  // Klawisze jako całe fragmenty — w wąskim polu przechodzą do drugiego wiersza w całości,
  // zamiast urywać „nowa | linia”.
  const parts = $derived(t(sendOnEnter ? 'composer.hint' : 'composer.hintCtrl').split(' · '));
  const notice = $derived(parts[0] ?? '');
  const keys = $derived(parts.slice(1));
  const SEP = ' · ';
  // Numer zapytania — odpowiedź na starszy szkic nie nadpisuje nowszej.
  let seq = 0;

  $effect(() => {
    const text = draft.trim();
    const mine = ++seq;
    if (!text || text.length > 160 || text.includes('\n') || attachments > 0) {
      local = null;
      return;
    }
    const timer = setTimeout(() => {
      app.client.localCommands.test(text).then(
        (r) => {
          if (mine !== seq) return;
          const answered = r.source === 'builtin' || r.source === 'custom' || r.source === 'quiz';
          local = answered ? (r.name ?? t('composer.localQuiz')) : null;
        },
        () => {
          if (mine === seq) local = null;
        },
      );
    }, 300);
    return () => clearTimeout(timer);
  });
</script>

<p class="hint">
  {notice}{#each keys as key (key)}{SEP}<span class="key">{key}</span>{/each}{#if local}{SEP}<strong
      class="local">{t('composer.local', { name: local })}</strong
    >{/if}
</p>

<style>
  .hint {
    color: var(--alfa-color-text-subtle);
    font-size: var(--alfa-font-size-xs);
    text-align: center;
  }
  .key,
  .local {
    white-space: nowrap;
  }
  .local {
    color: var(--alfa-color-text-muted);
    font-weight: var(--alfa-weight-semibold);
  }
</style>
