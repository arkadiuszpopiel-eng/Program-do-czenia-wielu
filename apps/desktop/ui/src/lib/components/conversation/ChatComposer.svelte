<!--
  Composer rozmowy: szkic per sesja, @agentka i /komendy z podpowiedziami (listbox), chipy agentki
  i profilu modelu, historia Ctrl+↑/↓, ↑ w pustym polu = edytuj ostatnią, Stop w trakcie strumienia.
  Załączniki: spinacz (natywny dialog), wklejenie plików/obrazu (czyta rdzeń ze schowka systemowego),
  przeciągnięcie (w Tauri ścieżki zna tylko powłoka) — wysyłane z turą jako artefakty sesji.
-->
<script lang="ts">
  import {
    Chip,
    Composer,
    Menu,
    MicButton,
    agentIds,
    agents,
    type AgentId,
    type MenuItem,
  } from '@alfa/ui-kit';
  import type { ModelProfile } from '../../api/types';
  import {
    SentHistory,
    addressedAgent,
    applyCompletion,
    findTrigger,
    type Trigger,
  } from '../../logic/composer';
  import { fuzzyRank } from '../../logic/fuzzy';
  import { lastUserTurn } from '../../logic/turn-tree';
  import { useApp } from '../../state/context';
  import { runCommand } from '../../state/commands';
  import type { ConversationState } from '../../state/conversation.svelte';
  import { AttachmentsState } from '../../state/attachments.svelte';
  import { carriesFiles, hintsFrom } from '../../logic/attachments';
  import ComposerAttachments from './ComposerAttachments.svelte';
  import ComposerSuggest, { type Suggestion } from './ComposerSuggest.svelte';

  interface Props {
    conv: ConversationState | null;
  }

  let { conv }: Props = $props();
  const app = useApp();
  const { t } = app.i18n;

  const COMMANDS: readonly { id: string; run: () => void }[] = [
    { id: 'obsada', run: () => runCommand(app, 'action.cast') },
    { id: 'model', run: () => (profileMenuOpen = true) },
    { id: 'pamiec', run: () => runCommand(app, 'panel.memory') },
    { id: 'eksport', run: () => app.activeId && void app.exportSession(app.activeId) },
    { id: 'ustawienia', run: () => runCommand(app, 'settings.open') },
    { id: 'skupienie', run: () => runCommand(app, 'view.focus') },
    { id: 'nowa', run: () => runCommand(app, 'session.new') },
  ];
  const COMMAND_WORD: Readonly<Record<string, string>> = { pamiec: 'pamięć' };

  const history = new SentHistory();
  const listId = $props.id();
  let textarea = $state<HTMLTextAreaElement | null>(null);
  let trigger = $state<Trigger | null>(null);
  let active = $state(0);
  let chosenAgent = $state<AgentId | null>(null);
  let profile = $state<ModelProfile | null>(null);
  let profileMenuOpen = $state(false);

  const att = new AttachmentsState(app);
  $effect(() => {
    void att.load(app.activeId);
  });
  $effect(() =>
    app.client.attachments.watchDrag((phase) => {
      att.dragging = phase === 'enter';
      if (phase === 'drop') void att.drop([]);
    }),
  );

  const draft = $derived(app.activeId ? (app.sessions.drafts[app.activeId] ?? '') : '');
  const setDraft = (text: string): void => app.setDraft(text);

  const effectiveProfile = $derived<ModelProfile>(
    profile ??
      (app.system?.profile === 'local'
        ? 'local'
        : (app.str('models.default_profile', 'hybrid') as ModelProfile)),
  );
  const agentList = agentIds.map((id) => ({ id, name: agents[id].name }));

  const suggestions = $derived.by((): Suggestion[] => {
    if (!trigger) return [];
    if (trigger.kind === '@') {
      return fuzzyRank(agentList, trigger.query, (a) => ({ label: a.name })).map(({ item }) => ({
        id: item.id,
        label: `@${item.name}`,
        hint: (app.agents[app.activeId ?? '']?.find((a) => a.id === item.id)?.role_ids ?? [])
          .map((r) => app.i18n.tk(`role.${r}`))
          .join(', '),
        insert: item.name,
        agent: item.id,
      }));
    }
    return fuzzyRank(COMMANDS, trigger.query, (c) => ({ label: COMMAND_WORD[c.id] ?? c.id })).map(
      ({ item }) => ({
        id: item.id,
        label: `/${COMMAND_WORD[item.id] ?? item.id}`,
        hint: app.i18n.tk(`composer.cmd.${item.id}`),
        insert: COMMAND_WORD[item.id] ?? item.id,
      }),
    );
  });
  const open = $derived(suggestions.length > 0);

  function refreshTrigger() {
    const el = textarea;
    trigger = el ? findTrigger(el.value, el.selectionStart ?? el.value.length) : null;
    active = 0;
  }

  function accept(s: Suggestion) {
    if (!trigger || !textarea) return;
    const next = applyCompletion(draft, trigger, s.insert);
    setDraft(next.text);
    trigger = null;
    const el = textarea;
    requestAnimationFrame(() => el.setSelectionRange(next.caret, next.caret));
  }

  function onkeydown(event: KeyboardEvent) {
    if (open) {
      const n = suggestions.length;
      if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
        event.preventDefault();
        active = (active + (event.key === 'ArrowDown' ? 1 : -1) + n) % n;
        return;
      }
      if (event.key === 'Enter' || event.key === 'Tab') {
        const s = suggestions[active];
        if (s) {
          event.preventDefault();
          accept(s);
        }
        return;
      }
      if (event.key === 'Escape') {
        event.preventDefault();
        event.stopPropagation();
        trigger = null;
        return;
      }
    }
    if (event.ctrlKey && (event.key === 'ArrowUp' || event.key === 'ArrowDown')) {
      const next = event.key === 'ArrowUp' ? history.prev(draft) : history.next();
      if (next !== null) {
        event.preventDefault();
        setDraft(next);
      }
      return;
    }
    if (event.key === 'ArrowUp' && !event.ctrlKey && draft === '' && conv) {
      const last = lastUserTurn(conv.path);
      if (last) {
        event.preventDefault();
        app.editingTurnId = last.id;
      }
    }
  }

  /** Odrzucone wysłanie nie kasuje szkicu: wraca do sesji, z której wyszło (gdy pole puste). */
  async function submit(text: string) {
    const origin = app.activeId;
    try {
      await deliver(text);
    } catch (error) {
      const id = origin ?? app.activeId;
      if (id && !app.sessions.drafts[id]) app.setDraft(text, id);
      app.toasts.show({
        kind: 'error',
        message: error instanceof Error ? error.message : String(error),
      });
    }
  }

  async function deliver(text: string) {
    if (text) history.push(text);
    trigger = null;
    const files = att.ids;
    const command = /^\/(\S+)\s*$/u.exec(text);
    if (command?.[1]) {
      const word = command[1].toLowerCase();
      const match = COMMANDS.find((c) => c.id === word || COMMAND_WORD[c.id] === word);
      if (match) {
        match.run();
        return;
      }
    }
    const run = app.runs.active(app.activeId);
    if (run && app.activeId && files.length === 0) {
      try {
        await app.client.agents.steer(app.activeId, text);
        app.toasts.show({
          kind: 'info',
          message: t('composer.steered', { name: agents[run.agent].name }),
        });
        return;
      } catch {
        // Zadanie właśnie się skończyło — wiadomość idzie zwykłą drogą.
      }
    }
    if (!conv) await app.newSession();
    const target = conv ?? app.conversation;
    if (!target) return;
    await target.send(text, chosenAgent ?? addressedAgent(text, agentList), profile, files);
    att.sent(files);
  }

  /** Pliki/obraz bez tekstu → rdzeń czyta schowek systemowy; tekst wkleja pole jak zwykle. */
  function onpaste(event: ClipboardEvent) {
    const data = event.clipboardData;
    if (!data || data.types.includes('text/plain')) return;
    if (data.files.length > 0 || [...data.items].some((i) => i.kind === 'file')) {
      event.preventDefault();
      void att.paste();
    }
  }

  function dragOver(event: DragEvent) {
    if (!carriesFiles(event.dataTransfer?.types ? [...event.dataTransfer.types] : null)) return;
    event.preventDefault();
    att.dragging = true;
  }

  function dropFiles(event: DragEvent) {
    const files = event.dataTransfer?.files;
    if (!files || files.length === 0) return;
    event.preventDefault();
    void att.drop(hintsFrom(files));
  }

  function toggleMic() {
    runCommand(app, 'voice.mic');
  }

  const agentItems = $derived<MenuItem[]>([
    {
      id: 'auto',
      label: t('composer.agentAutoHint'),
      checked: chosenAgent === null,
      onSelect: () => (chosenAgent = null),
    },
    ...agentIds.map((id) => ({
      id,
      label: agents[id].name,
      checked: chosenAgent === id,
      onSelect: () => (chosenAgent = id),
    })),
  ]);
  const profileItems = $derived<MenuItem[]>(
    (['local', 'hybrid', 'cloud'] as const).map((p) => ({
      id: p,
      label: t(`profile.${p}`),
      checked: effectiveProfile === p,
      disabled: p !== 'local' && app.system?.keys_configured === false,
      onSelect: () => (profile = p),
    })),
  );
  const micLabels = $derived({
    off: { label: t('mic.off'), hint: t('mic.hint.off') },
    listening: { label: t('mic.listening'), hint: t('mic.hint.on') },
    hearing: { label: t('mic.hearing'), hint: t('mic.hint.on') },
    processing: { label: t('mic.processing'), hint: t('mic.hint.processing') },
    speaking: { label: t('mic.speaking'), hint: t('mic.hint.speaking') },
    muted: { label: t('mic.muted'), hint: t('mic.hint.muted') },
    dnd: { label: t('mic.dnd'), hint: t('mic.hint.dnd') },
  });
  const sendOnEnter = $derived(app.bool('composer.enter_sends', true));
  /** Trwające zadanie agentki — wiadomość trafia do niej w trakcie (steering, PLAN §9.6). */
  const steering = $derived(app.runs.active(app.activeId));
</script>

<div
  class="composer-area"
  class:dragging={att.dragging}
  role="group"
  aria-label={t('att.zone')}
  ondragenter={dragOver}
  ondragover={dragOver}
  ondragleave={(e) => {
    if (!e.currentTarget.contains(e.relatedTarget as Node | null)) att.dragging = false;
  }}
  ondrop={dropFiles}
>
  {#if att.dragging}<p class="drop-hint" aria-hidden="true">{t('att.dropHere')}</p>{/if}
  <Composer
    bind:value={() => draft, setDraft}
    bind:textarea
    {sendOnEnter}
    busy={Boolean(conv?.streaming)}
    allowEmpty={att.items.length > 0}
    onattach={() => void att.pick()}
    lang={app.i18n.locale}
    placeholder={steering
      ? t('composer.steerPlaceholder', { name: agents[steering.agent].name })
      : t('composer.placeholder')}
    labels={{
      field: t('composer.label'),
      send: t('composer.send'),
      stop: t('composer.stop'),
      attach: t('att.attach'),
    }}
    fieldAttrs={{
      'aria-autocomplete': 'list',
      'aria-controls': open ? listId : undefined,
      'aria-activedescendant': open ? `${listId}-${active}` : undefined,
      id: 'alfa-composer',
      onpaste,
    }}
    {onkeydown}
    oninput={refreshTrigger}
    onsubmit={(text) => submit(text)}
    onstop={() => void conv?.stop()}
  >
    {#snippet above()}
      <ComposerAttachments {att} profile={effectiveProfile} turns={conv?.path ?? []} />
      {#if open}
        <ComposerSuggest
          {suggestions}
          {active}
          {listId}
          label={t(trigger?.kind === '@' ? 'composer.mentions' : 'composer.commands')}
          onaccept={accept}
        />
      {/if}
    {/snippet}
    {#snippet chips()}
      <Menu
        items={agentItems}
        label={t('composer.agent', {
          name: chosenAgent ? agents[chosenAgent].name : t('composer.agentAuto'),
        })}
        align="end"
      >
        {#snippet trigger(props)}
          <Chip
            {...props}
            agent={chosenAgent ?? undefined}
            size="sm"
            onclick={props.onclick}
            label={t('composer.agent', {
              name: chosenAgent ? agents[chosenAgent].name : t('composer.agentAuto'),
            })}
          >
            {chosenAgent ? agents[chosenAgent].name : t('composer.agentAuto')} ▾
          </Chip>
        {/snippet}
      </Menu>
      <Menu
        items={profileItems}
        label={t('composer.profile', { profile: t(`profile.${effectiveProfile}`) })}
        bind:open={profileMenuOpen}
      >
        {#snippet trigger(props)}
          <Chip
            {...props}
            size="sm"
            onclick={props.onclick}
            label={t('composer.profile', { profile: t(`profile.${effectiveProfile}`) })}
          >
            {t(`profile.${effectiveProfile}`)} ▾
          </Chip>
        {/snippet}
      </Menu>
    {/snippet}
    {#snippet trailing()}
      <MicButton state={app.micState} showLabel={false} onclick={toggleMic} labels={micLabels} />
    {/snippet}
  </Composer>
  <p class="hint">{t(sendOnEnter ? 'composer.hint' : 'composer.hintCtrl')}</p>
</div>

<style>
  .composer-area {
    position: relative;
    display: flex;
    flex-direction: column;
    gap: var(--alfa-space-1);
  }
  .hint {
    color: var(--alfa-color-text-subtle);
    font-size: var(--alfa-font-size-xs);
    text-align: center;
  }
  .dragging :global(.composer) {
    outline: var(--alfa-size-focus-ring) dashed var(--alfa-color-focus);
    outline-offset: 2px;
  }
  .drop-hint {
    color: var(--alfa-color-text-muted);
    font-size: var(--alfa-font-size-sm);
    text-align: center;
  }
</style>
