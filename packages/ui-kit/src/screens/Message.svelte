<script lang="ts">
  import Copy from '@lucide/svelte/icons/copy';
  import Volume2 from '@lucide/svelte/icons/volume-2';
  import RotateCcw from '@lucide/svelte/icons/rotate-ccw';
  import Pencil from '@lucide/svelte/icons/pencil';
  import Ellipsis from '@lucide/svelte/icons/ellipsis';
  import FileText from '@lucide/svelte/icons/file-text';
  import Search from '@lucide/svelte/icons/search';
  import Wrench from '@lucide/svelte/icons/wrench';
  import Undo2 from '@lucide/svelte/icons/undo-2';
  import Avatar from '../components/Avatar.svelte';
  import Chip from '../components/Chip.svelte';
  import IconButton from '../components/IconButton.svelte';
  import ApprovalCard from '../components/ApprovalCard.svelte';
  import { agents } from '../tokens';
  import type { ChatMessage } from '../types';

  interface Props {
    message: ChatMessage;
  }

  let { message }: Props = $props();
  const isUser = $derived(message.author === 'user');
  const agent = $derived(message.author === 'user' ? undefined : message.author);
  const stepIcon = (icon: string | undefined) =>
    icon === 'search' ? Search : icon === 'edit' ? Pencil : icon === 'terminal' ? Wrench : FileText;
  const fmt = (ms: number) =>
    ms >= 1000 ? `${(ms / 1000).toFixed(1).replace('.', ',')} s` : `${ms} ms`;
</script>

<article
  class="msg"
  class:user={isUser}
  aria-label={isUser ? 'Twoja wiadomość' : `Wiadomość agentki ${agent ? agents[agent].name : ''}`}
>
  <header class="head">
    {#if agent}
      <Avatar {agent} size={24} speaking={message.streaming} />
      <span class="name" style:color="var(--alfa-agent-{agent})">{agents[agent].name}</span>
      {#if message.role}<Chip size="sm" {agent}>{message.role}</Chip>{/if}
    {:else}
      <span class="name">Ty</span>
    {/if}
    <time class="time" title={message.time}>{message.time}</time>
    {#if message.variants}
      <span
        class="variants"
        aria-label="Wariant {message.variants.index} z {message.variants.total}"
      >
        ‹ {message.variants.index}/{message.variants.total} ›
      </span>
    {/if}
    <div class="actions" role="toolbar" aria-label="Akcje wiadomości">
      <IconButton label="Kopiuj" size="sm"><Copy size={14} strokeWidth={1.5} /></IconButton>
      {#if agent}
        <IconButton label="Przeczytaj na głos" size="sm"
          ><Volume2 size={14} strokeWidth={1.5} /></IconButton
        >
        <IconButton label="Ponów" size="sm"><RotateCcw size={14} strokeWidth={1.5} /></IconButton>
      {:else}
        <IconButton label="Edytuj i wyślij ponownie" size="sm"
          ><Pencil size={14} strokeWidth={1.5} /></IconButton
        >
      {/if}
      <IconButton label="Więcej" size="sm"><Ellipsis size={14} strokeWidth={1.5} /></IconButton>
    </div>
  </header>

  {#if message.steps?.length}
    <ul class="steps" aria-label="Kroki narzędzi">
      {#each message.steps as step (step.id)}
        {@const Icon = stepIcon(step.icon)}
        <li class="step">
          <Icon size={14} strokeWidth={1.5} aria-hidden="true" />
          <span class="step-label">{step.label}</span>
          {#if step.durationMs !== undefined}<span class="step-time">{fmt(step.durationMs)}</span
            >{/if}
          {#if step.undoable}
            <button type="button" class="undo"
              ><Undo2 size={12} strokeWidth={1.5} aria-hidden="true" /> Cofnij</button
            >
          {/if}
        </li>
      {/each}
    </ul>
  {/if}

  <div class="body" class:streaming={message.streaming}>
    {#each message.text.split('\n\n') as para, i (i)}
      <p>{para}</p>
    {/each}
    {#if message.streaming}<span class="caret" aria-hidden="true"></span>{/if}
  </div>

  {#if message.approval && agent}
    <div class="approval">
      <ApprovalCard {agent} {...message.approval} />
    </div>
  {/if}
</article>

<style>
  .msg {
    display: flex;
    flex-direction: column;
    gap: var(--alfa-space-2);
    padding: var(--alfa-space-3) 0;
  }
  .head {
    display: flex;
    align-items: center;
    gap: var(--alfa-space-2);
    min-height: 24px;
  }
  .name {
    font-weight: var(--alfa-weight-semibold);
    font-size: var(--alfa-font-size-sm);
  }
  .time,
  .variants {
    color: var(--alfa-color-text-subtle);
    font-size: var(--alfa-font-size-xs);
    font-variant-numeric: tabular-nums;
  }
  .actions {
    display: inline-flex;
    margin-left: auto;
    opacity: 0;
    transition: opacity var(--alfa-duration-fast) var(--alfa-ease-out);
  }
  .msg:hover .actions,
  .msg:focus-within .actions {
    opacity: 1;
  }
  .body {
    padding-left: 32px;
    max-width: 72ch;
  }
  .user .body {
    padding: var(--alfa-space-2) var(--alfa-space-3);
    margin-left: 32px;
    border-radius: var(--alfa-radius-card);
    background: var(--alfa-color-surface2);
  }
  .body p + p {
    margin-top: var(--alfa-space-2);
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
  .steps {
    margin: 0 0 0 32px;
    padding: 0;
    list-style: none;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .step {
    display: flex;
    align-items: center;
    gap: var(--alfa-space-2);
    min-height: 24px;
    color: var(--alfa-color-text-muted);
    font-size: var(--alfa-font-size-sm);
  }
  .step-time {
    color: var(--alfa-color-text-subtle);
    font-size: var(--alfa-font-size-xs);
    font-variant-numeric: tabular-nums;
  }
  .undo {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    height: 22px;
    padding: 0 var(--alfa-space-2);
    border: 1px solid var(--alfa-color-border);
    border-radius: var(--alfa-radius-full);
    background: var(--alfa-color-surface);
    color: var(--alfa-color-text-muted);
    font-size: var(--alfa-font-size-xs);
  }
  .undo:hover {
    color: var(--alfa-color-text);
  }
  .approval {
    margin-left: 32px;
    max-width: 72ch;
  }
</style>
