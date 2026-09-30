<!--
  Wiadomość: nagłówek (awatar + imię + chip roli, czas względny, warianty ‹ i/n ›), pasek akcji,
  myślenie (zwinięte, z czasem), kroki narzędzi, bloki treści z Rust, karta „czeka na zatwierdzenie",
  błąd z „Ponów / inny model", „Kontynuuj" dla uciętej odpowiedzi, edycja → nowa gałąź.
-->
<script lang="ts">
  import {
    Avatar,
    Button,
    Chip,
    SanitizedHtml,
    Skeleton,
    VariantSwitcher,
    agents,
  } from '@alfa/ui-kit';
  import Brain from '@lucide/svelte/icons/brain';
  import Clock from '@lucide/svelte/icons/clock';
  import type { Turn } from '../../api/types';
  import { now } from '../../state/clock.svelte';
  import { useApp } from '../../state/context';
  import type { ConversationState } from '../../state/conversation.svelte';
  import CodeBlock from './CodeBlock.svelte';
  import MessageApproval from './MessageApproval.svelte';
  import MessageError from './MessageError.svelte';
  import MessageActions from './MessageActions.svelte';
  import ToolSteps from './ToolSteps.svelte';

  interface Props {
    turn: Turn;
    conv: ConversationState;
    position: number;
    count: number;
    highlighted?: boolean;
  }

  let { turn, conv, position, count, highlighted = false }: Props = $props();
  const app = useApp();
  const { t } = app.i18n;

  const agent = $derived(turn.author === 'user' ? null : turn.author);
  const name = $derived(agent ? agents[agent].name : t('conv.you'));
  const siblings = $derived(conv.siblings(turn));
  const annotation = $derived(conv.annotations[turn.id]);
  const streaming = $derived(turn.status === 'streaming');
  const editing = $derived(app.editingTurnId === turn.id);
  let bodyEl = $state<HTMLElement | null>(null);
  let pinned = $state(false);
  let draft = $state('');
  let slow = $state(false);

  // Szkielet dopiero po 300 ms bez treści (PLAN §14.8: bez migających spinnerów).
  $effect(() => {
    if (!streaming || turn.blocks.length > 0) {
      slow = false;
      return;
    }
    const handle = setTimeout(() => (slow = true), 300);
    return () => clearTimeout(handle);
  });

  $effect(() => {
    if (editing) draft = turn.text;
  });

  function startEdit() {
    app.editingTurnId = turn.id;
  }

  async function submitEdit() {
    const text = draft.trim();
    app.editingTurnId = null;
    if (text && text !== turn.text) await conv.editAndResend(turn, text);
  }

  function editKey(event: KeyboardEvent) {
    if (event.key === 'Escape') {
      event.preventDefault();
      event.stopPropagation();
      app.editingTurnId = null;
    } else if (event.key === 'Enter' && !event.shiftKey) {
      event.preventDefault();
      void submitEdit();
    }
  }

  function focusOnMount(node: HTMLTextAreaElement) {
    node.focus();
    node.setSelectionRange(node.value.length, node.value.length);
  }
</script>

{#if annotation?.hidden}
  <article
    class="hidden-note"
    aria-posinset={position}
    aria-setsize={count}
    aria-label={t('conv.hidden')}
  >
    <span>{t('conv.hidden')}</span>
    <Button size="sm" variant="ghost" onclick={() => void conv.setHidden(turn, false)}
      >{t('conv.unhide')}</Button
    >
  </article>
{:else}
  <article
    class="msg"
    class:user={!agent}
    class:pinned
    class:highlighted
    aria-posinset={position}
    aria-setsize={count}
    aria-label={agent
      ? t('conv.messageAgent', { name, time: app.i18n.dateTime(turn.created_at) })
      : t('conv.messageYou', { time: app.i18n.dateTime(turn.created_at) })}
    oncontextmenu={(e) => {
      e.preventDefault();
      pinned = !pinned;
    }}
    style:--accent={agent ? `var(--alfa-agent-${agent})` : undefined}
  >
    <header class="head">
      {#if agent}
        <Avatar {agent} size={24} speaking={streaming} />
        <span class="name agent-name">{name}</span>
        {#if turn.role_id}<Chip size="sm" {agent}>{app.i18n.tk(`role.${turn.role_id}`)}</Chip>{/if}
      {:else}
        <span class="name">{name}</span>
        {#if turn.addressed_to}
          <span class="addressed"
            >{t('conv.addressed', { name: agents[turn.addressed_to].name })}</span
          >
        {/if}
      {/if}
      <time class="time" datetime={turn.created_at} title={app.i18n.dateTime(turn.created_at)}>
        {app.i18n.relative(turn.created_at, now())}
      </time>
      {#if siblings.total > 1}
        <VariantSwitcher
          index={siblings.index}
          total={siblings.total}
          label={t(agent ? 'conv.variant' : 'conv.branch', {
            index: siblings.index,
            total: siblings.total,
          })}
          prevLabel={t('conv.prev')}
          nextLabel={t('conv.next')}
          onprev={() => conv.selectVariant(turn, -1)}
          onnext={() => conv.selectVariant(turn, 1)}
        />
      {/if}
      <div class="bar">
        <MessageActions
          {turn}
          {conv}
          rating={annotation?.rating ?? null}
          {bodyEl}
          onedit={startEdit}
        />
      </div>
    </header>

    {#if turn.thinking}
      <p class="thinking" class:active={turn.thinking.active}>
        <Brain size={14} strokeWidth={1.5} aria-hidden="true" />
        {t(turn.thinking.active ? 'conv.thinkingActive' : 'conv.thinking', {
          duration: app.i18n.duration(turn.thinking.duration_ms),
        })}
      </p>
    {/if}

    {#if turn.tools.length}<ToolSteps steps={turn.tools} />{/if}

    {#if editing}
      <div class="edit">
        <label class="alfa-visually-hidden" for="edit-{turn.id}">{t('msg.editLabel')}</label>
        <textarea
          id="edit-{turn.id}"
          bind:value={draft}
          rows="3"
          onkeydown={editKey}
          use:focusOnMount></textarea>
        <div class="edit-actions">
          <Button size="sm" variant="secondary" onclick={() => (app.editingTurnId = null)}
            >{t('common.cancel')}</Button
          >
          <Button size="sm" variant="primary" onclick={submitEdit}>{t('msg.editSend')}</Button>
        </div>
      </div>
    {:else}
      <div class="body" bind:this={bodyEl}>
        {#each turn.blocks as block (block.index)}
          {#if block.kind === 'code'}
            <CodeBlock {block} turnId={turn.id} settled={!streaming} />
          {:else}
            <SanitizedHtml html_sanitized={block.html_sanitized} />
          {/if}
        {/each}
        {#if streaming && turn.blocks.length > 0}<span class="caret" aria-hidden="true"></span>{/if}
        {#if slow && !turn.thinking?.active}<Skeleton
            lines={3}
            label={t('conv.loadingAnswer')}
          />{/if}
      </div>
    {/if}

    {#if turn.status === 'queued'}
      <p class="note">
        <Clock size={14} strokeWidth={1.5} aria-hidden="true" />
        {t('conv.queued')}
      </p>
    {:else if turn.status === 'cancelled'}
      <p class="note">{t('conv.cancelled')}</p>
    {/if}

    {#if turn.approval && agent}
      <div class="indent"><MessageApproval {agent} approval={turn.approval} /></div>
    {/if}

    {#if turn.error}
      <div class="indent"><MessageError {turn} error={turn.error} {conv} /></div>
    {/if}

    {#if turn.usage && agent && !streaming}
      <p class="usage">
        {t('msg.usage', {
          model: turn.usage.model,
          input: app.i18n.int(turn.usage.input_tokens),
          output: app.i18n.int(turn.usage.output_tokens),
          cost: app.i18n.money(turn.usage.cost),
          latency: app.i18n.duration(turn.usage.latency_ms),
        })}
      </p>
    {/if}

    {#if turn.truncated && turn.status === 'complete'}
      <div class="continue">
        <span>{t('conv.truncated')}</span>
        <Button size="sm" variant="secondary" onclick={() => void conv.continueTurn(turn)}
          >{t('conv.continue')}</Button
        >
      </div>
    {/if}
  </article>
{/if}

<style>
  .msg {
    display: flex;
    flex-direction: column;
    gap: var(--alfa-space-2);
    padding: var(--alfa-space-3) var(--alfa-space-2);
    border-radius: var(--alfa-radius-card);
  }
  .highlighted {
    outline: 2px solid var(--alfa-color-focus);
    outline-offset: 2px;
  }
  .head {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: var(--alfa-space-2);
    min-height: 28px;
  }
  .name {
    font-weight: var(--alfa-weight-semibold);
    font-size: var(--alfa-font-size-sm);
  }
  .agent-name {
    color: var(--accent);
  }
  .addressed,
  .time {
    color: var(--alfa-color-text-subtle);
    font-size: var(--alfa-font-size-xs);
    font-variant-numeric: tabular-nums;
  }
  .bar {
    margin-left: auto;
    opacity: 0;
    transition: opacity var(--alfa-duration-fast) var(--alfa-ease-out);
  }
  .msg:hover .bar,
  .msg:focus-within .bar,
  .pinned .bar,
  .msg:hover .usage,
  .msg:focus-within .usage,
  .pinned .usage {
    opacity: 1;
  }
  /* Szczegóły wywołania (model, tokeny, koszt, opóźnienie) — widoczne przy najechaniu / fokusie. */
  .usage {
    color: var(--alfa-color-text-muted);
    font-size: var(--alfa-font-size-xs);
    font-variant-numeric: tabular-nums;
    opacity: 0;
    transition: opacity var(--alfa-duration-fast) var(--alfa-ease-out);
  }
  .body,
  .thinking,
  .usage,
  .note,
  .edit,
  .indent,
  .continue,
  .msg > :global(.tools) {
    margin-left: 32px;
  }
  .user .body {
    padding: var(--alfa-space-2) var(--alfa-space-3);
    border-radius: var(--alfa-radius-card);
    background: var(--alfa-color-surface2);
  }
  .thinking,
  .note {
    display: inline-flex;
    align-items: center;
    gap: var(--alfa-space-1);
    color: var(--alfa-color-text-muted);
    font-size: var(--alfa-font-size-sm);
  }
  .thinking.active {
    animation: breathe 1.4s var(--alfa-ease-in-out) infinite alternate;
  }
  @keyframes breathe {
    to {
      opacity: 0.55;
    }
  }
  .caret {
    display: inline-block;
    width: 2px;
    height: 1em;
    margin-left: 2px;
    vertical-align: text-bottom;
    background: currentColor;
    animation: blink 1s steps(2) infinite;
  }
  @keyframes blink {
    to {
      opacity: 0;
    }
  }
  .edit {
    display: flex;
    flex-direction: column;
    gap: var(--alfa-space-2);
  }
  .edit textarea {
    width: 100%;
    padding: var(--alfa-space-2) var(--alfa-space-3);
    border: 1px solid var(--alfa-color-border-strong);
    border-radius: var(--alfa-radius-card);
    background: var(--alfa-color-surface);
    resize: vertical;
  }
  .edit-actions,
  .continue {
    display: flex;
    align-items: center;
    justify-content: flex-end;
    gap: var(--alfa-space-2);
  }
  .continue {
    justify-content: flex-start;
    color: var(--alfa-color-text-muted);
    font-size: var(--alfa-font-size-sm);
  }
  .hidden-note {
    display: flex;
    align-items: center;
    gap: var(--alfa-space-2);
    padding: var(--alfa-space-2) var(--alfa-space-2) var(--alfa-space-2) 40px;
    color: var(--alfa-color-text-subtle);
    font-size: var(--alfa-font-size-xs);
  }
  @media (prefers-reduced-motion: reduce) {
    .caret,
    .thinking.active {
      animation: none;
    }
  }
</style>
