<!-- Pasek akcji wiadomości (po najechaniu, fokusie lub prawym przyciskiem) — PLAN §14.8. -->
<script lang="ts">
  import { IconButton, Menu, type MenuItem } from '@alfa/ui-kit';
  import Copy from '@lucide/svelte/icons/copy';
  import Volume2 from '@lucide/svelte/icons/volume-2';
  import RotateCcw from '@lucide/svelte/icons/rotate-ccw';
  import Pencil from '@lucide/svelte/icons/pencil';
  import ThumbsUp from '@lucide/svelte/icons/thumbs-up';
  import ThumbsDown from '@lucide/svelte/icons/thumbs-down';
  import Ellipsis from '@lucide/svelte/icons/ellipsis';
  import type { RememberScope, Turn } from '../../api/types';
  import { useApp } from '../../state/context';
  import type { ConversationState } from '../../state/conversation.svelte';

  interface Props {
    turn: Turn;
    conv: ConversationState;
    rating: 'up' | 'down' | null;
    bodyEl: HTMLElement | null;
    onedit: () => void;
  }

  let { turn, conv, rating, bodyEl, onedit }: Props = $props();
  const app = useApp();
  const { t } = app.i18n;
  const isUser = $derived(turn.author === 'user');
  const done = $derived(turn.status !== 'streaming');

  async function copy(plain: boolean) {
    const text = plain ? (bodyEl?.innerText ?? turn.text) : turn.text;
    await navigator.clipboard.writeText(text);
    app.toasts.show({ kind: 'success', message: t('common.copied'), timeoutMs: 2500 });
  }

  async function remember(scope: RememberScope) {
    await app.client.turns.remember(turn.id, scope);
    app.toasts.show({ kind: 'success', message: t('msg.remembered') });
  }

  const more = $derived.by((): MenuItem[] => {
    const items: MenuItem[] = [
      { id: 'copy-text', label: t('msg.copyText'), onSelect: () => void copy(true) },
    ];
    if (!isUser) {
      items.push(
        {
          id: 'regen-local',
          label: t('msg.regenerateLocal'),
          disabled: !done,
          onSelect: () => void conv.regenerate(turn, 'local'),
        },
        {
          id: 'regen-cloud',
          label: t('msg.regenerateCloud'),
          disabled: !done,
          onSelect: () => void conv.regenerate(turn, 'cloud'),
        },
      );
    }
    const scopes: RememberScope[] = ['session', 'project', 'global', 'agent'];
    scopes.forEach((scope, i) =>
      items.push({
        id: `remember-${scope}`,
        label: t(`msg.remember.${scope}`),
        separatorBefore: i === 0,
        onSelect: () => void remember(scope),
      }),
    );
    items.push(
      {
        id: 'details',
        label: t('msg.details'),
        separatorBefore: true,
        onSelect: () => app.showTimelineFor(turn.id),
      },
      { id: 'hide', label: t('msg.hide'), onSelect: () => void conv.setHidden(turn, true) },
    );
    return items;
  });
</script>

<div class="actions" role="toolbar" aria-label={t('msg.actions')}>
  <IconButton label={t('msg.copy')} size="sm" onclick={() => copy(false)}>
    <Copy size={14} strokeWidth={1.5} />
  </IconButton>
  {#if isUser}
    <IconButton label={t('msg.edit')} size="sm" onclick={onedit}>
      <Pencil size={14} strokeWidth={1.5} />
    </IconButton>
  {:else}
    <IconButton
      label={t('msg.readAloud')}
      size="sm"
      onclick={() => void app.client.turns.readAloud(turn.id)}
    >
      <Volume2 size={14} strokeWidth={1.5} />
    </IconButton>
    <IconButton
      label={t('msg.regenerate')}
      size="sm"
      disabled={!done}
      onclick={() => void conv.regenerate(turn)}
    >
      <RotateCcw size={14} strokeWidth={1.5} />
    </IconButton>
    <IconButton
      label={t('msg.rateUp')}
      size="sm"
      pressed={rating === 'up'}
      onclick={() => void conv.rate(turn, 'up')}
    >
      <ThumbsUp size={14} strokeWidth={1.5} />
    </IconButton>
    <IconButton
      label={t('msg.rateDown')}
      size="sm"
      pressed={rating === 'down'}
      onclick={() => void conv.rate(turn, 'down')}
    >
      <ThumbsDown size={14} strokeWidth={1.5} />
    </IconButton>
  {/if}
  <Menu items={more} label={t('msg.more')}>
    {#snippet trigger(props)}
      <IconButton {...props} label={t('msg.more')} size="sm">
        <Ellipsis size={14} strokeWidth={1.5} />
      </IconButton>
    {/snippet}
  </Menu>
</div>

<style>
  .actions {
    display: inline-flex;
    align-items: center;
  }
</style>
