<script lang="ts">
  import TitleBar from './TitleBar.svelte';
  import SessionList from './SessionList.svelte';
  import Message from './Message.svelte';
  import Panel from '../components/Panel.svelte';
  import Composer from '../components/Composer.svelte';
  import ActivityCapsule from '../components/ActivityCapsule.svelte';
  import MicButton from '../components/MicButton.svelte';
  import Chip from '../components/Chip.svelte';
  import Avatar from '../components/Avatar.svelte';
  import Toast from '../components/Toast.svelte';
  import { agentIds, agents, type AgentId } from '../tokens';
  import type { ActivityInfo, ChatMessage, MicState, SessionItem } from '../types';

  interface Props {
    project: string;
    session: string;
    sessions: readonly SessionItem[];
    messages: readonly ChatMessage[];
    activity?: ActivityInfo;
    micState?: MicState;
    leftOpen?: boolean;
    rightOpen?: boolean;
    /** Toast w prawym dolnym rogu (np. „Cofnij" po akcji fs.*). */
    toast?: { message: string; actionLabel?: string };
    onsubmit?: (text: string) => void;
    onstop?: () => void;
    onsettings?: () => void;
  }

  let {
    project,
    session,
    sessions,
    messages,
    activity,
    micState = 'off',
    leftOpen = $bindable(true),
    rightOpen = $bindable(true),
    toast,
    onsubmit,
    onstop,
    onsettings,
  }: Props = $props();

  const activeAgent = $derived<AgentId | undefined>(
    activity?.agent ??
      (messages.find((m) => m.streaming && m.author !== 'user')?.author as AgentId | undefined),
  );
  let draft = $state('');
  let tab = $state('agents');
  const tabs = [
    { id: 'agents', label: 'Agentki' },
    { id: 'timeline', label: 'Oś czasu' },
    { id: 'files', label: 'Pliki' },
    { id: 'memory', label: 'Pamięć' },
    { id: 'screen', label: 'Ekran' },
    { id: 'voice', label: 'Głos' },
  ];
  const roles: Record<AgentId, string> = {
    alfa: 'Koordynatorka',
    beta: 'Mówczyni',
    gama: 'Analityczka',
    delta: 'Wykonawczyni',
  };
</script>

<div class="shell" class:left-open={leftOpen} class:right-open={rightOpen}>
  <TitleBar
    {project}
    {session}
    {activeAgent}
    {leftOpen}
    {rightOpen}
    ontoggleleft={() => (leftOpen = !leftOpen)}
    ontoggleright={() => (rightOpen = !rightOpen)}
    {onsettings}
  />
  <div class="main">
    {#if leftOpen}
      <aside class="left">
        <SessionList {sessions} />
      </aside>
    {/if}
    <section class="conv" aria-label="Rozmowa">
      <div class="scroll">
        <div class="column">
          {#each messages as message (message.id)}
            <Message {message} />
          {/each}
        </div>
      </div>
      {#if activity}
        <div class="capsule">
          <ActivityCapsule {...activity} {onstop} />
        </div>
      {/if}
      <div class="composer">
        <Composer bind:value={draft} {onsubmit} onattach={() => {}}>
          {#snippet chips()}
            <Chip agent="alfa" size="sm" onclick={() => {}} label="Adresat: Alfa">Alfa ▾</Chip>
            <Chip size="sm" onclick={() => {}} label="Profil modelu: Hybryda">Hybryda ▾</Chip>
          {/snippet}
          {#snippet trailing()}
            <MicButton state={micState} agent={activeAgent} showLabel={false} />
          {/snippet}
        </Composer>
      </div>
    </section>
    {#if rightOpen}
      <aside class="right">
        <Panel title="Panel" {tabs} bind:activeTab={tab} onclose={() => (rightOpen = false)}>
          {#if tab === 'agents'}
            <ul class="agents">
              {#each agentIds as id (id)}
                <li class="agent">
                  <Avatar agent={id} size={32} speaking={activeAgent === id} />
                  <span class="agent-text">
                    <span class="agent-name">{agents[id].name}</span>
                    <span class="agent-role">{roles[id]}</span>
                  </span>
                  <span class="agent-state">{activeAgent === id ? 'pracuje' : 'gotowa'}</span>
                </li>
              {/each}
            </ul>
          {:else}
            <p class="placeholder">
              Zawartość karty „{tabs.find((t) => t.id === tab)?.label}" — makieta w kolejnej
              iteracji.
            </p>
          {/if}
        </Panel>
      </aside>
    {/if}
  </div>
  {#if toast}
    <div class="toasts">
      <Toast
        kind="success"
        message={toast.message}
        actionLabel={toast.actionLabel}
        onaction={() => {}}
        onclose={() => {}}
      />
    </div>
  {/if}
</div>

<style>
  .shell {
    display: flex;
    flex-direction: column;
    height: 100%;
    min-height: 0;
    background: var(--alfa-color-bg);
    color: var(--alfa-color-text);
    position: relative;
  }
  .main {
    flex: 1;
    display: grid;
    grid-template-columns: auto minmax(0, 1fr) auto;
    min-height: 0;
  }
  .left {
    width: 240px;
    min-height: 0;
  }
  .right {
    width: 320px;
    min-height: 0;
  }
  .conv {
    display: flex;
    flex-direction: column;
    min-width: 0;
    min-height: 0;
  }
  .scroll {
    flex: 1;
    min-height: 0;
    overflow: auto;
    overscroll-behavior: contain;
  }
  .column {
    width: min(var(--alfa-size-reading-column-wide), 100%);
    margin: 0 auto;
    padding: var(--alfa-space-4) var(--alfa-space-4) var(--alfa-space-8);
  }
  .capsule {
    display: flex;
    justify-content: center;
    padding: 0 var(--alfa-space-4) var(--alfa-space-2);
  }
  .composer {
    width: min(var(--alfa-size-reading-column-wide), 100%);
    margin: 0 auto;
    padding: 0 var(--alfa-space-4) var(--alfa-space-4);
  }
  .agents {
    display: flex;
    flex-direction: column;
    gap: var(--alfa-space-2);
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .agent {
    display: flex;
    align-items: center;
    gap: var(--alfa-space-3);
    min-height: 40px;
  }
  .agent-text {
    display: flex;
    flex-direction: column;
    flex: 1;
    min-width: 0;
  }
  .agent-name {
    font-weight: var(--alfa-weight-semibold);
    font-size: var(--alfa-font-size-sm);
  }
  .agent-role,
  .agent-state {
    color: var(--alfa-color-text-muted);
    font-size: var(--alfa-font-size-xs);
  }
  .placeholder {
    color: var(--alfa-color-text-muted);
    font-size: var(--alfa-font-size-sm);
  }
  .toasts {
    position: absolute;
    right: var(--alfa-space-4);
    bottom: 72px;
  }
  /* Responsywność §14.2: poniżej 1100 px panele nakładają się na treść jako szuflady. */
  @media (max-width: 1100px) {
    .right {
      position: absolute;
      top: var(--alfa-size-titlebar);
      right: 0;
      bottom: 0;
      z-index: 2;
      box-shadow: var(--alfa-shadow-3);
    }
  }
  @media (max-width: 720px) {
    .left {
      position: absolute;
      top: var(--alfa-size-titlebar);
      left: 0;
      bottom: 0;
      z-index: 2;
      box-shadow: var(--alfa-shadow-3);
    }
  }
</style>
