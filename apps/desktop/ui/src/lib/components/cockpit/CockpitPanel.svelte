<script lang="ts">
  import { personInfo } from '@alfa/ui-kit';
  import Radar from '@lucide/svelte/icons/radar';
  import MessagesSquare from '@lucide/svelte/icons/messages-square';
  import Users from '@lucide/svelte/icons/users';
  import ListChecks from '@lucide/svelte/icons/list-checks';
  import Mic from '@lucide/svelte/icons/mic';
  import Cpu from '@lucide/svelte/icons/cpu';
  import ShieldCheck from '@lucide/svelte/icons/shield-check';
  import Smartphone from '@lucide/svelte/icons/smartphone';
  import Globe from '@lucide/svelte/icons/globe';
  import Lazy from '../shell/Lazy.svelte';
  import { useApp } from '../../state/context';
  import { openPersonControls } from '../../logic/person-controls';
  import { cockpitCopy } from './copy';
  import './cockpit.css';
  const app = useApp();
  const copy = $derived(cockpitCopy(app.i18n.locale));
  const cast = $derived(app.activeId ? (app.agents[app.activeId] ?? []) : []);
  let remote = $state(false);
  let launch = $state<HTMLButtonElement | null>(null);
  let systems = $state(false);
  let systemsLaunch = $state<HTMLButtonElement | null>(null);
  const loadSystems = () => import('./SystemsPanel.svelte');
  const closeSystems = () => {
    systems = false;
    requestAnimationFrame(() => systemsLaunch?.focus());
  };
  const loadRemote = () => import('./RemotePreview.svelte');
  const closeRemote = () => {
    remote = false;
    requestAnimationFrame(() => launch?.focus());
  };
  const tiles = $derived([
    {
      id: 'sessions',
      icon: MessagesSquare,
      label: copy.sessions,
      run: () => app.layout.toggleLeft(app.activeId),
    },
    { id: 'agents', icon: Users, label: copy.agents, run: () => app.openPanel('agents') },
    { id: 'tasks', icon: ListChecks, label: copy.tasks, run: () => app.openPanel('tasks') },
    { id: 'voice', icon: Mic, label: copy.voice, run: () => app.openSettings('voice') },
    { id: 'models', icon: Cpu, label: copy.models, run: () => app.openSettings('models') },
    {
      id: 'access',
      icon: ShieldCheck,
      label: copy.access,
      run: () => app.openSettings('permissions'),
    },
  ]);
</script>

<section class="cockpit" aria-label={copy.title}>
  <header class="cockpit-head">
    <div>
      <p class="cockpit-eyebrow">JARVIS <span aria-hidden="true">/</span> ALFA</p>
      <h2>{copy.heading}</h2>
      <p class="cockpit-session">
        <span>{copy.session}</span>
        {app.sessions.active?.title ?? copy.empty}
      </p>
    </div>
    <div class="cockpit-orb" aria-hidden="true"><i></i><Radar size={32} strokeWidth={1} /></div>
  </header>
  <nav class="cockpit-tiles" aria-label={copy.navigation}>
    {#each tiles as tile (tile.id)}
      <button type="button" data-cockpit-tile={tile.id} onclick={tile.run}>
        <tile.icon size={20} strokeWidth={1.5} aria-hidden="true" /><span>{tile.label}</span>
      </button>
    {/each}
  </nav>
  <div class="cockpit-footer">
    {#if cast.length}
      <div class="cockpit-cast" role="group" aria-label={copy.crew}>
        {#each cast as person (person.id)}
          <button
            type="button"
            onclick={() => {
              openPersonControls(app, person.id);
            }}
          >
            <span class="cockpit-state" data-status={person.status} aria-hidden="true"></span>
            {personInfo(person.id).name}<span class="cockpit-agent-status"
              >{app.i18n.t(`agentStatus.${person.status}`)}</span
            >
          </button>
        {/each}
      </div>
    {/if}
    <button
      type="button"
      class="cockpit-phone cockpit-systems"
      bind:this={systemsLaunch}
      onclick={() => (systems = true)}
      ><Globe size={20} strokeWidth={1.5} aria-hidden="true" /><span>{copy.systems}</span></button
    >
    <button
      type="button"
      class="cockpit-phone"
      bind:this={launch}
      aria-label={copy.phone}
      title={copy.phone}
      onclick={() => (remote = true)}
    >
      <Smartphone size={20} strokeWidth={1.5} aria-hidden="true" />
    </button>
  </div>
  {#if remote}<Lazy
      load={loadRemote}
      props={{ onclose: closeRemote }}
      label={app.i18n.t('common.loading')}
    />{/if}
  {#if systems}<Lazy
      load={loadSystems}
      props={{ onclose: closeSystems }}
      label={app.i18n.t('common.loading')}
    />{/if}
</section>
