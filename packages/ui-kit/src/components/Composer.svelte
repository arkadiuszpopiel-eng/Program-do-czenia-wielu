<script lang="ts">
  import type { Snippet } from 'svelte';
  import Paperclip from '@lucide/svelte/icons/paperclip';
  import Send from '@lucide/svelte/icons/send';
  import IconButton from './IconButton.svelte';

  interface Props {
    /** Treść (bindable). */
    value?: string;
    placeholder?: string;
    /** Enter wysyła, Shift+Enter nowa linia (odwracalne w ustawieniach — §14.8). */
    sendOnEnter?: boolean;
    disabled?: boolean;
    maxLines?: number;
    onsubmit?: (text: string) => void;
    onattach?: () => void;
    /** Chipy wyboru agentki / profilu (po lewej od mikrofonu). */
    chips?: Snippet;
    /** Przycisk mikrofonu (MicButton) — po prawej. */
    trailing?: Snippet;
  }

  let {
    value = $bindable(''),
    placeholder = 'Napisz… (@agentka, /komenda)',
    sendOnEnter = true,
    disabled = false,
    maxLines = 12,
    onsubmit,
    onattach,
    chips,
    trailing,
  }: Props = $props();

  let textarea = $state<HTMLTextAreaElement | null>(null);
  const LINE_PX = 21; // 14 px × 1,5
  const PAD_PX = 16;
  const canSend = $derived(value.trim().length > 0 && !disabled);

  /** Auto-wzrost 1–maxLines linii (aktualizacja przy każdej zmianie wartości). */
  $effect(() => {
    void value;
    const el = textarea;
    if (!el) return;
    el.style.height = 'auto';
    const max = LINE_PX * maxLines + PAD_PX;
    el.style.height = `${Math.min(el.scrollHeight, max)}px`;
    el.style.overflowY = el.scrollHeight > max ? 'auto' : 'hidden';
  });

  function submit() {
    if (!canSend) return;
    const text = value.trim();
    onsubmit?.(text);
    value = '';
    textarea?.focus();
  }

  function onkeydown(event: KeyboardEvent) {
    if (event.key !== 'Enter' || event.isComposing) return;
    const wantsNewline = sendOnEnter ? event.shiftKey : !event.ctrlKey;
    if (wantsNewline) return;
    event.preventDefault();
    submit();
  }
</script>

<form
  class="composer"
  onsubmit={(e) => {
    e.preventDefault();
    submit();
  }}
>
  {#if onattach}
    <IconButton label="Dołącz plik" onclick={onattach} {disabled}>
      <Paperclip size={18} strokeWidth={1.5} />
    </IconButton>
  {/if}
  <label class="field">
    <span class="alfa-visually-hidden">Wiadomość</span>
    <textarea
      bind:this={textarea}
      bind:value
      rows="1"
      {placeholder}
      {disabled}
      {onkeydown}
      spellcheck="true"
      lang="pl"
      aria-keyshortcuts={sendOnEnter ? 'Enter' : 'Control+Enter'}></textarea>
  </label>
  {#if chips}
    <div class="chips">{@render chips()}</div>
  {/if}
  {#if trailing}{@render trailing()}{/if}
  <IconButton label="Wyślij" type="submit" disabled={!canSend} class="send">
    <Send size={18} strokeWidth={1.5} />
  </IconButton>
</form>

<style>
  .composer {
    display: flex;
    align-items: flex-end;
    gap: var(--alfa-space-1);
    padding: var(--alfa-space-1);
    border: 1px solid var(--alfa-color-border);
    border-radius: var(--alfa-radius-card);
    background: var(--alfa-color-surface);
    box-shadow: var(--alfa-shadow-1);
  }
  .composer:focus-within {
    border-color: var(--alfa-color-border-strong);
  }
  .field {
    flex: 1;
    min-width: 0;
    display: flex;
  }
  textarea {
    flex: 1;
    min-height: 32px;
    padding: 8px 8px;
    border: 0;
    resize: none;
    background: transparent;
    color: var(--alfa-color-text);
    line-height: 21px;
    font-size: var(--alfa-font-size-md);
  }
  textarea::placeholder {
    color: var(--alfa-color-text-subtle);
  }
  textarea:focus {
    outline: none;
  }
  .chips {
    display: flex;
    align-items: center;
    gap: var(--alfa-space-1);
    padding-bottom: 2px;
  }
  :global(.composer .send:not(:disabled)) {
    color: var(--alfa-color-text);
  }
</style>
