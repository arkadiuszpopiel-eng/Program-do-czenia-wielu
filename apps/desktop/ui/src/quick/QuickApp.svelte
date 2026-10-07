<!--
  Szybkie pytanie (ui-quick, makieta 5): okno 640 px, pole, odpowiedź rozwija się pod spodem.
  Enter — zapytaj; po odpowiedzi Enter w pustym polu — „otwórz w pełnym oknie"; Esc — zamknij.
-->
<script lang="ts">
  import { untrack } from 'svelte';
  import Avatar from '@alfa/ui-kit/components/Avatar.svelte';
  import SanitizedHtml from '@alfa/ui-kit/components/SanitizedHtml.svelte';
  import type { AlfaClient } from '../lib/api/client';
  import { errorText } from '../lib/api/command-error';
  import type { Locale } from '../lib/api/types';
  import type { AlfaEvent } from '../lib/api/types-system';
  import { RafBatcher } from '../lib/logic/raf-batcher';
  import { QuickSession } from './quick-session.svelte';
  import { quickText, type QuickKey } from './strings';

  interface Props {
    client: AlfaClient;
    locale?: Locale;
  }

  const props: Props = $props();
  const client = untrack(() => props.client);
  const locale = $derived(props.locale ?? 'pl');
  const t = (key: QuickKey, message?: string) => quickText(locale, key, message);

  const quick = new QuickSession(client);
  let question = $state('');
  let failure = $state<string | null>(null);
  let input = $state<HTMLInputElement | null>(null);

  const batcher = new RafBatcher<AlfaEvent>((batch) => quick.apply(batch));

  $effect(() => {
    const off = client.subscribe((batch) => batcher.push(...batch));
    input?.focus();
    return () => {
      off();
      batcher.dispose();
    };
  });

  async function ask() {
    const text = question.trim();
    if (!text) return;
    question = '';
    failure = null;
    try {
      await quick.ask(text);
    } catch (error) {
      // Odrzucone pytanie nie ginie: wraca do pola, jeśli nie wpisano nic nowego.
      if (!question) question = text;
      failure = errorText(error);
    }
  }

  /** Komenda okna (zamknij, otwórz w pełnym oknie); odrzucona — komunikat pod polem. */
  async function windowAction(action: () => Promise<unknown>) {
    try {
      await action();
    } catch (error) {
      failure = errorText(error);
    }
  }

  const expand = (id: string) => windowAction(() => client.quick.expandToMain(id));

  function onkeydown(event: KeyboardEvent) {
    if (event.key === 'Escape') {
      event.preventDefault();
      void windowAction(() => client.quick.hide());
    } else if (event.key === 'Enter' && !event.isComposing) {
      event.preventDefault();
      if (question.trim()) void ask();
      else if (quick.sessionId) void expand(quick.sessionId);
    }
  }

  const answer = $derived(quick.answer);
  const sessionId = $derived(quick.sessionId);
  const streaming = $derived(answer?.status === 'streaming');
</script>

<main class="quick" aria-label={t('title')}>
  <div class="bar">
    <Avatar agent="alfa" size={28} speaking={streaming} />
    <label class="alfa-visually-hidden" for="quick-input">{t('label')}</label>
    <input
      id="quick-input"
      bind:this={input}
      bind:value={question}
      placeholder={t('placeholder')}
      autocomplete="off"
      spellcheck="true"
      lang={locale}
      {onkeydown}
    />
    <kbd>Esc</kbd>
  </div>
  {#if answer || quick.queued || failure}
    <section class="answer" aria-label={t('answer')} aria-live="polite" aria-busy={streaming}>
      {#if failure}
        <p class="error" role="alert">{t('error', failure)}</p>
      {:else if quick.queued}
        <p class="muted">{t('queued')}</p>
      {:else if answer}
        {#if answer.thinking?.active && answer.blocks.length === 0}<p class="muted">
            {t('thinking')}
          </p>{/if}
        {#each answer.blocks as block (block.index)}
          <SanitizedHtml html_sanitized={block.html_sanitized} />
        {/each}
        {#if answer.error}<p class="error" role="alert">{t('error', answer.error.message)}</p>{/if}
      {/if}
    </section>
  {/if}
  <footer class="foot">
    <span>{sessionId && !streaming ? t('hintExpand') : t('hintAsk')}</span>
    {#if sessionId && !streaming}
      <button type="button" class="open" onclick={() => sessionId && void expand(sessionId)}
        >{t('open')}</button
      >
    {/if}
  </footer>
</main>

<style>
  .quick {
    display: flex;
    flex-direction: column;
    width: min(640px, 100vw);
    max-height: 100vh;
    margin: 0 auto;
    border: 1px solid var(--alfa-color-border);
    border-radius: var(--alfa-radius-overlay);
    background: var(--alfa-color-surface);
    box-shadow: var(--alfa-shadow-3);
    overflow: hidden;
  }
  .bar {
    display: flex;
    align-items: center;
    gap: var(--alfa-space-3);
    padding: var(--alfa-space-3) var(--alfa-space-4);
  }
  input {
    flex: 1;
    min-width: 0;
    height: 36px;
    border: 0;
    background: transparent;
    color: var(--alfa-color-text);
    font-size: var(--alfa-font-size-lg);
  }
  input:focus {
    outline: none;
  }
  .bar:focus-within {
    box-shadow: inset 0 -2px 0 var(--alfa-color-focus);
  }
  kbd {
    padding: 1px 6px;
    border: 1px solid var(--alfa-color-border);
    border-radius: 4px;
    background: var(--alfa-color-surface2);
    color: var(--alfa-color-text-muted);
    font-size: var(--alfa-font-size-xs);
  }
  .answer {
    max-height: 60vh;
    padding: var(--alfa-space-3) var(--alfa-space-4);
    overflow: auto;
    border-top: 1px solid var(--alfa-color-border);
    user-select: text;
  }
  .muted {
    color: var(--alfa-color-text-muted);
  }
  .error {
    color: var(--alfa-color-error);
  }
  .foot {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--alfa-space-2);
    padding: var(--alfa-space-2) var(--alfa-space-4);
    border-top: 1px solid var(--alfa-color-border);
    background: var(--alfa-color-bg);
    color: var(--alfa-color-text-muted);
    font-size: var(--alfa-font-size-xs);
  }
  .open {
    min-height: 24px;
    padding: 0 var(--alfa-space-2);
    border: 1px solid var(--alfa-color-border);
    border-radius: var(--alfa-radius-control);
    background: var(--alfa-color-surface);
    color: var(--alfa-color-text);
    font-size: var(--alfa-font-size-xs);
    font-weight: var(--alfa-weight-semibold);
  }
</style>
