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
  import { attempt } from '../../state/attempt';
  import { TEMPLATE_COUNT_MAX, TEMPLATE_TEXT_MAX, templateNameFrom } from '../../logic/templates';
  import { useApp } from '../../state/context';
  import type { ConversationState } from '../../state/conversation.svelte';
  import { exportConversation } from '../../state/exports';

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
    try {
      await navigator.clipboard.writeText(text);
      app.toasts.show({ kind: 'success', message: t('common.copied'), timeoutMs: 2500 });
    } catch {
      app.toasts.show({ kind: 'warning', message: t('msg.copyFailed') });
    }
  }

  async function remember(scope: RememberScope) {
    const ok = await attempt(app.toasts, () => app.client.turns.remember(turn.id, scope));
    if (ok) app.toasts.show({ kind: 'success', message: t('msg.remembered') });
  }

  /** Akcja na wiadomości: błąd rdzenia → toast (nie ginie po cichu). */
  function run(action: () => Promise<unknown>): void {
    void attempt(app.toasts, action);
  }

  /** Prośba o tłumaczenie odpowiedzi (P5): nowa wiadomość w tej rozmowie, sam przekład. */
  function translation(lang: 'en' | 'pl'): string {
    return t(lang === 'en' ? 'msg.translatePromptEn' : 'msg.translatePromptPl', {
      text: turn.text,
    });
  }

  /** „Zapisz jako szablon” (wiadomość użytkownika): dopisanie do listy szablonów w rdzeniu. */
  async function saveTemplate(): Promise<void> {
    const text = turn.text.trim();
    if (text.length > TEMPLATE_TEXT_MAX) {
      app.toasts.show({ kind: 'warning', message: t('tpl.tooLong', { max: TEMPLATE_TEXT_MAX }) });
      return;
    }
    const list = await app.client.templates.list();
    if (list.length >= TEMPLATE_COUNT_MAX) {
      app.toasts.show({ kind: 'warning', message: t('tpl.full', { max: TEMPLATE_COUNT_MAX }) });
      return;
    }
    // Nazwa ma najwyżej 40 znaków i numer — mieści się w limicie rdzenia (60).
    const name = templateNameFrom(
      text,
      list.map((x) => x.name),
      t('tpl.defaultName'),
    );
    await app.client.templates.save([...list, { id: '', name, text }]);
    app.toasts.show({ kind: 'success', message: t('tpl.savedFrom', { name }) });
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
          onSelect: () => run(() => conv.regenerate(turn, 'local')),
        },
        {
          id: 'regen-cloud',
          label: t('msg.regenerateCloud'),
          disabled: !done,
          onSelect: () => run(() => conv.regenerate(turn, 'cloud')),
        },
      );
    }
    if (!isUser) {
      items.push(
        {
          id: 'translate-en',
          label: t('msg.translateEn'),
          disabled: !done,
          onSelect: () => run(() => conv.send(translation('en'), null, null)),
        },
        {
          id: 'translate-pl',
          label: t('msg.translatePl'),
          disabled: !done,
          onSelect: () => run(() => conv.send(translation('pl'), null, null)),
        },
      );
      items.push({
        id: 'pin-board',
        label: t('board.pin'),
        disabled: !done,
        onSelect: () =>
          run(async () => {
            await app.client.board.pin(conv.sessionId, turn.id);
            app.toasts.show({ kind: 'success', message: t('board.pinned') });
          }),
      });
    }
    if (isUser) {
      items.push({
        id: 'save-template',
        label: t('tpl.fromMessage'),
        disabled: !turn.text.trim(),
        onSelect: () => run(saveTemplate),
      });
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
      { id: 'hide', label: t('msg.hide'), onSelect: () => run(() => conv.setHidden(turn, true)) },
      {
        id: 'export-md',
        label: t('exp.messageMarkdown'),
        separatorBefore: true,
        onSelect: () => void exportConversation(app, conv.sessionId, 'markdown', turn.id),
      },
      {
        id: 'export-html',
        label: t('exp.messageHtml'),
        onSelect: () => void exportConversation(app, conv.sessionId, 'html', turn.id),
      },
      {
        id: 'export-json',
        label: t('exp.messageJson'),
        onSelect: () => void exportConversation(app, conv.sessionId, 'json', turn.id),
      },
    );
    return items;
  });
</script>

<div class="actions" role="toolbar" aria-label={t('msg.actions')}>
  <IconButton label={t('msg.copy')} size="sm" onclick={() => void copy(false)}>
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
      onclick={() => run(() => app.client.turns.readAloud(turn.id))}
    >
      <Volume2 size={14} strokeWidth={1.5} />
    </IconButton>
    <IconButton
      label={t('msg.regenerate')}
      size="sm"
      disabled={!done}
      onclick={() => run(() => conv.regenerate(turn))}
    >
      <RotateCcw size={14} strokeWidth={1.5} />
    </IconButton>
    <IconButton
      label={t('msg.rateUp')}
      size="sm"
      pressed={rating === 'up'}
      onclick={() => run(() => conv.rate(turn, 'up'))}
    >
      <ThumbsUp size={14} strokeWidth={1.5} />
    </IconButton>
    <IconButton
      label={t('msg.rateDown')}
      size="sm"
      pressed={rating === 'down'}
      onclick={() => run(() => conv.rate(turn, 'down'))}
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
