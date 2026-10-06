<script lang="ts">
  import type { Snippet } from 'svelte';
  import type { HTMLTextareaAttributes } from 'svelte/elements';
  import Paperclip from '@lucide/svelte/icons/paperclip';
  import Send from '@lucide/svelte/icons/send';
  import Square from '@lucide/svelte/icons/square';
  import IconButton from './IconButton.svelte';
  import { submitDraft } from './draft';

  interface Labels {
    field: string;
    send: string;
    attach: string;
    stop: string;
  }

  interface Props {
    /** Treść (bindable). */
    value?: string;
    placeholder?: string;
    /** Enter wysyła, Shift+Enter nowa linia (odwracalne w ustawieniach — §14.8). */
    sendOnEnter?: boolean;
    disabled?: boolean;
    maxLines?: number;
    /** Trwa generowanie: zamiast „Wyślij" pokazuje „Stop". */
    busy?: boolean;
    /** Wysłanie dozwolone przy pustym polu (np. same załączniki). */
    allowEmpty?: boolean;
    lang?: string;
    labels?: Partial<Labels>;
    /** Element pola (bindable) — np. do ustawienia kursora po podpowiedzi. */
    textarea?: HTMLTextAreaElement | null;
    /** Dodatkowe atrybuty pola (np. ARIA combobox dla podpowiedzi @/ ). */
    fieldAttrs?: HTMLTextareaAttributes;
    /** Wywoływane przed domyślną obsługą klawiszy; `preventDefault()` ją pomija. */
    onkeydown?: (event: KeyboardEvent) => void;
    oninput?: (event: Event) => void;
    /** Wysłanie; odrzucona obietnica przywraca treść do pustego pola (błąd zgłasza wywołujący). */
    onsubmit?: (text: string) => unknown;
    onstop?: () => void;
    onattach?: () => void;
    /** Chipy wyboru agentki / profilu (po lewej od mikrofonu). */
    chips?: Snippet;
    /** Przycisk mikrofonu (MicButton) — po prawej. */
    trailing?: Snippet;
    /** Treść nad polem (np. lista podpowiedzi). */
    above?: Snippet;
  }

  let {
    value = $bindable(''),
    placeholder = 'Napisz… (@agentka, /komenda)',
    sendOnEnter = true,
    disabled = false,
    maxLines = 12,
    busy = false,
    allowEmpty = false,
    lang = 'pl',
    labels = {},
    textarea = $bindable(null),
    fieldAttrs = {},
    onkeydown,
    oninput,
    onsubmit,
    onstop,
    onattach,
    chips,
    trailing,
    above,
  }: Props = $props();

  const text = $derived<Labels>({
    field: 'Wiadomość',
    send: 'Wyślij',
    attach: 'Dołącz plik',
    stop: 'Zatrzymaj generowanie',
    ...labels,
  });
  const LINE_PX = 21; // 14 px × 1,5
  const PAD_PX = 16;
  const canSend = $derived((value.trim().length > 0 || allowEmpty) && !disabled);

  /** Auto-wzrost 1–maxLines linii, najwyżej ~40% wysokości okna (§14.8). */
  $effect(() => {
    void value;
    const el = textarea;
    if (!el) return;
    el.style.height = 'auto';
    const max = Math.min(LINE_PX * maxLines + PAD_PX, Math.max(80, window.innerHeight * 0.4));
    el.style.height = `${Math.min(el.scrollHeight, max)}px`;
    el.style.overflowY = el.scrollHeight > max ? 'auto' : 'hidden';
  });

  function submit() {
    if (!canSend) return;
    submitDraft({ get: () => value, set: (text) => (value = text) }, onsubmit);
    textarea?.focus();
  }

  function handleKeydown(event: KeyboardEvent) {
    onkeydown?.(event);
    if (event.defaultPrevented) return;
    if (event.key !== 'Enter' || event.isComposing) return;
    const wantsNewline = sendOnEnter ? event.shiftKey : !event.ctrlKey;
    if (wantsNewline) return;
    event.preventDefault();
    submit();
  }
</script>

<div class="wrap">
  {#if above}{@render above()}{/if}
  <form
    class="composer"
    onsubmit={(e) => {
      e.preventDefault();
      submit();
    }}
  >
    {#if onattach}
      <IconButton label={text.attach} onclick={onattach} {disabled}>
        <Paperclip size={18} strokeWidth={1.5} />
      </IconButton>
    {/if}
    <label class="field">
      <span class="alfa-visually-hidden">{text.field}</span>
      <textarea
        {...fieldAttrs}
        bind:this={textarea}
        bind:value
        rows="1"
        {placeholder}
        {disabled}
        {lang}
        {oninput}
        onkeydown={handleKeydown}
        spellcheck="true"
        aria-keyshortcuts={sendOnEnter ? 'Enter' : 'Control+Enter'}></textarea>
    </label>
    {#if chips}
      <div class="chips">{@render chips()}</div>
    {/if}
    {#if trailing}{@render trailing()}{/if}
    {#if busy && onstop}
      <IconButton label={text.stop} onclick={onstop} class="stop">
        <Square size={16} strokeWidth={2} />
      </IconButton>
    {:else}
      <IconButton label={text.send} type="submit" disabled={!canSend} class="send">
        <Send size={18} strokeWidth={1.5} />
      </IconButton>
    {/if}
  </form>
</div>

<style>
  .wrap {
    position: relative;
  }
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
  /* Fokus pola widoczny na całej ramce (2 px, WCAG 2.4.11). */
  .composer:has(textarea:focus) {
    outline: var(--alfa-size-focus-ring) solid var(--alfa-color-focus);
    outline-offset: 1px;
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
  :global(.composer .send:not(:disabled)),
  :global(.composer .stop) {
    color: var(--alfa-color-text);
  }
</style>
